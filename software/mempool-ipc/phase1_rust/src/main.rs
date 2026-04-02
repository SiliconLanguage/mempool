//! Bare-metal entry point for MemPool-IPC Phase 4 Cluster Verification.
//!
//! Initializes the SPSC lock-free queue in a statically allocated,
//! `#[repr(align(64))]` shared L1 TCDM region. Branches execution by
//! `mhartid`: Core 0 produces, Core 1 consumes. Both synchronize via
//! `hw_barrier()` backed by the conditionally compiled `hardware_spin_wait`:
//!
//! - `target_feature = "zawrs"` → NTL.PALL + LR.W.AQ + WRS.NTO
//! - fallback → acquire load + `core::hint::spin_loop()`

use core::sync::atomic::{AtomicU32, Ordering};

use crate::primitives::{core_id, num_cores, nop, hardware_spin_wait};

// ---------------------------------------------------------------------------
// Hardware constants (must match Phase 0 / MemPool CFLAGS)
// ---------------------------------------------------------------------------
const RING_SIZE: usize = 1024;

// ---------------------------------------------------------------------------
// L1 TCDM data structures — layout-compatible with Phase 0 C structs
// ---------------------------------------------------------------------------

/// A single slot in the lock-free ring buffer.
///
/// Alignment to 64 bytes (CACHELINE_SIZE) prevents false sharing
/// on the MemPool L1 TCDM interconnect.
#[repr(C, align(64))]
struct QueueSlot {
    sequence: AtomicU32,
    data: u32,
}

/// Cacheline-aligned u32 wrapper (4-byte value + 60 bytes padding).
#[repr(C, align(64))]
struct CachelineU32(u32);

/// The SPSC hardware queue, placed in shared L1 TCDM.
///
/// Layout (verified against C):
/// - ring:              offset 0x00000, size 0x10000 (1024 × 64)
/// - next_producer_seq: offset 0x10000, aligned to 64
/// - next_consumer_seq: offset 0x10040, aligned to 64
/// - total size:        0x10080 (65664 bytes)
#[repr(C)]
struct HardwareQueue {
    ring: [QueueSlot; RING_SIZE],
    next_producer_seq: CachelineU32,
    next_consumer_seq: CachelineU32,
}

// ---------------------------------------------------------------------------
// RVWMO atomic queue operations
// ---------------------------------------------------------------------------

/// Publish a message with release semantics.
///
/// Writes `data` to the slot, then atomically stores `seq` into the
/// slot's sequence counter with `Ordering::Release`.
#[inline(always)]
fn publish_message(q: &mut HardwareQueue, seq: u32, msg: u32) {
    let index = (seq as usize) & (RING_SIZE - 1);
    q.ring[index].data = msg;
    q.ring[index].sequence.store(seq, Ordering::Release);
}

/// Attempt to consume a message with acquire semantics.
///
/// Returns `Some(data)` if `slot.sequence >= expected_seq`.
#[inline(always)]
#[allow(dead_code)]
fn consume_message(q: &HardwareQueue, expected_seq: u32) -> Option<u32> {
    let index = (expected_seq as usize) & (RING_SIZE - 1);
    let current_seq = q.ring[index].sequence.load(Ordering::Acquire);
    if current_seq >= expected_seq {
        Some(q.ring[index].data)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// L1 TCDM globals — placed in .l1 section by the linker (spike_globals.S)
// ---------------------------------------------------------------------------

extern "C" {
    #[link_name = "ipc_queue"]
    static mut IPC_QUEUE: HardwareQueue;

    #[link_name = "boot_barrier"]
    static BOOT_BARRIER: AtomicU32;

    /// HTIF tohost — Spike monitors this address for program exit.
    /// Write `(exit_code << 1) | 1` to trigger clean shutdown.
    #[link_name = "tohost"]
    static mut TOHOST: u64;

    #[link_name = "fromhost"]
    static mut FROMHOST: u64;
}

// ---------------------------------------------------------------------------
// Bare-metal spin barrier
// ---------------------------------------------------------------------------

/// Spin-barrier using SeqCst fetch_add + hardware_spin_wait.
///
/// `hardware_spin_wait` auto-selects at compile time:
/// - Zawrs: LR.W.AQ + WRS.NTO (zero crossbar traffic while suspended)
/// - Fallback: acquire load + spin_loop()
#[inline(never)]
fn hw_barrier(barrier: &AtomicU32, expected: u32) {
    barrier.fetch_add(1, Ordering::SeqCst);
    hardware_spin_wait(barrier, expected);
}

/// Busy-wait for `cycles` iterations (nop loop).
#[inline(never)]
fn hw_wait(cycles: u32) {
    for _ in 0..cycles {
        nop();
    }
}

// ---------------------------------------------------------------------------
// Entry point — called by crt0.S after stack/gp setup
// ---------------------------------------------------------------------------

/// The `main` symbol called by MemPool's `crt0.S` boot code.
///
/// # Safety
///
/// Called in bare-metal context: stack set by crt0.S, mhartid identifies
/// core, L1 TCDM present but uninitialized, no heap/allocator/OS.
#[no_mangle]
pub unsafe extern "C" fn main() -> i32 {
    let cid = core_id();
    let ncores = num_cores();

    // --- Core 0 initializes the queue ---
    if cid == 0 {
        let q = &raw mut IPC_QUEUE;
        for i in 0..RING_SIZE {
            (*q).ring[i].sequence.store(0, Ordering::Relaxed);
            (*q).ring[i].data = 0;
        }
        (*q).next_producer_seq.0 = 1;
        (*q).next_consumer_seq.0 = 1;
    }

    // Synchronize: wait for all cores to see initialized L1 TCDM
    hw_barrier(&*&raw const BOOT_BARRIER, ncores);

    // --- SPSC DATA PLANE TEST ---
    if cid == 0 {
        // ---- Producer (Core 0) ----
        let q = &mut *&raw mut IPC_QUEUE;
        let payload: u32 = 0xDEAD_BEEF;
        let seq = q.next_producer_seq.0;
        q.next_producer_seq.0 = seq + 1;

        hw_wait(100); // Guarantee consumer is spinning before publish
        publish_message(q, seq, payload);

    } else if cid == 1 {
        // ---- Consumer (Core 1) ----
        let q_ptr = &raw mut IPC_QUEUE;
        let expected_seq = (*q_ptr).next_consumer_seq.0;
        (*q_ptr).next_consumer_seq.0 = expected_seq + 1;
        let q = &*q_ptr;

        // Spin-wait on the sequence counter via hardware_spin_wait.
        // Compile-time dispatch:
        //   target_feature="zawrs" → NTL.PALL + LR.W.AQ + WRS.NTO
        //   fallback              → acquire load + spin_loop()
        let slot_idx = (expected_seq as usize) & (RING_SIZE - 1);
        hardware_spin_wait(&q.ring[slot_idx].sequence, expected_seq);

        // Acquire fence already provided by hardware_spin_wait; safe to read data
        let received = q.ring[slot_idx].data;
        if received != 0xDEAD_BEEF {
            return -1; // Data mismatch
        }
    }

    // Synchronize before exit
    hw_barrier(&*&raw const BOOT_BARRIER, ncores);

    // HTIF exit: Core 0 writes to tohost so Spike terminates cleanly.
    // Protocol: (exit_code << 1) | 1. For exit code 0, write 1.
    if cid == 0 {
        core::ptr::write_volatile(&raw mut FROMHOST as *mut u64, 0);
        core::ptr::write_volatile(&raw mut TOHOST as *mut u64, 1);
    }
    0
}

// ---------------------------------------------------------------------------
// Panic handler — required by #![no_std]
// ---------------------------------------------------------------------------

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        nop();
    }
}
