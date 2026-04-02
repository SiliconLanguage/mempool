//! MemPool-IPC Phase 1: Native Rust Wrapper
//!
//! `#![no_std]` bare-metal crate providing:
//! - Rust-native SPSC lock-free queue over MemPool's L1 TCDM
//! - Safe wrappers for the MemPool C runtime (barriers, allocator, MMIO)
//! - RVWMO acquire/release atomics
//!
//! Linked with MemPool's C runtime (`crt0.S`) via `extern "C" fn main`.

#![no_std]
#![no_main]

// -------------------------------------------------------------------------
// Modules
// -------------------------------------------------------------------------

#[allow(dead_code)]
mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

/// Bare-metal RISC-V primitives: CSR access, topology queries, timing.
pub mod primitives;

/// MMIO hardware register map: wake-up, EOC, cache control.
pub mod hw;

/// FFI bindings to MemPool synchronization barrier functions.
pub mod sync;

/// FFI bindings to MemPool dynamic memory allocator.
pub mod alloc_ffi;

use core::sync::atomic::{AtomicU32, Ordering};

use primitives::{core_id, num_cores, nop};

// ---------------------------------------------------------------------------
// Hardware constants (must match Phase 0 CFLAGS)
// ---------------------------------------------------------------------------
const RING_SIZE: usize = 1024;

// ---------------------------------------------------------------------------
// L1 TCDM data structures — layout-compatible with Phase 0 C structs
// ---------------------------------------------------------------------------

/// A single slot in the lock-free ring buffer.
///
/// C equivalent:
/// ```c
/// typedef struct __attribute__((aligned(64))) {
///     uint32_t sequence;
///     uint32_t data;
/// } QueueSlot;
/// ```
///
/// The `sequence` field is accessed atomically (acquire/release).
/// Alignment to 64 bytes (CACHELINE_SIZE) prevents false sharing
/// on the MemPool L1 TCDM interconnect.
#[repr(C, align(64))]
struct QueueSlot {
    sequence: AtomicU32,
    data: u32,
}

/// Cacheline-aligned u32 wrapper.
///
/// Replicates C's `uint32_t field __attribute__((aligned(64)))` for
/// struct members. The wrapper occupies 64 bytes total (4 byte value
/// + 60 bytes trailing padding from alignment).
#[repr(C, align(64))]
struct CachelineU32(u32);

/// The SPSC hardware queue, placed in shared L1 TCDM.
///
/// C equivalent:
/// ```c
/// typedef struct {
///     QueueSlot ring[1024];
///     uint32_t next_producer_seq __attribute__((aligned(64)));
///     uint32_t next_consumer_seq __attribute__((aligned(64)));
/// } HardwareQueue;
/// ```
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
/// slot's sequence counter with `Ordering::Release`. This ensures the
/// data write is visible before the sequence update on RVWMO.
///
/// Compiles to: `fence rw,w` + `sw` (release store).
#[inline(always)]
fn publish_message(q: &mut HardwareQueue, seq: u32, msg: u32) {
    let index = (seq as usize) & (RING_SIZE - 1);
    q.ring[index].data = msg;
    q.ring[index].sequence.store(seq, Ordering::Release);
}

/// Attempt to consume a message with acquire semantics.
///
/// Atomically loads the slot's sequence counter with `Ordering::Acquire`.
/// If the counter >= `expected_seq`, reads and returns the data.
///
/// Compiles to: `lw` + `fence r,rw` (acquire load).
#[inline(always)]
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
// L1 TCDM globals — placed in .l1 section by linker
// ---------------------------------------------------------------------------

extern "C" {
    #[link_name = "ipc_queue"]
    static mut IPC_QUEUE: HardwareQueue;

    #[link_name = "boot_barrier"]
    static BOOT_BARRIER: AtomicU32;
}

// ---------------------------------------------------------------------------
// Bare-metal spin barrier (seq_cst fetch_add + spin)
// ---------------------------------------------------------------------------

/// Spin-barrier using SeqCst fetch_add.
///
/// Compiles to: `fence iorw,ow` + `amoadd.w.aqrl` (identical to Phase 0 C).
#[inline(never)]
fn hw_barrier(barrier: &AtomicU32, expected: u32) {
    barrier.fetch_add(1, Ordering::SeqCst);
    while barrier.load(Ordering::SeqCst) < expected {
        nop();
    }
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

    // Wait for L1 initialization
    hw_barrier(&*&raw const BOOT_BARRIER, ncores);

    // --- SPSC DATA PLANE TEST ---
    if cid == 0 {
        let q = &mut *&raw mut IPC_QUEUE;
        let payload: u32 = 0xDEAD_BEEF;
        let seq = q.next_producer_seq.0;
        q.next_producer_seq.0 = seq + 1;

        hw_wait(100); // Guarantee consumer is spinning
        publish_message(q, seq, payload);
    } else if cid == 1 {
        let q_ptr = &raw mut IPC_QUEUE;
        let expected_seq = (*q_ptr).next_consumer_seq.0;
        (*q_ptr).next_consumer_seq.0 = expected_seq + 1;
        let q = &*q_ptr;

        loop {
            if let Some(received) = consume_message(q, expected_seq) {
                if received != 0xDEAD_BEEF {
                    return -1;
                }
                break;
            }
            nop();
        }
    }

    hw_barrier(&*&raw const BOOT_BARRIER, ncores);
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
