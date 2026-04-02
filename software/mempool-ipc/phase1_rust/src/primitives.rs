//! Bare-metal RISC-V primitives for MemPool.
//!
//! Re-implements the `static inline` functions from `runtime.h` that
//! cannot cross the FFI boundary (header-only, never compiled into a `.o`).
//! Also provides topology query functions derived from compile-time config.

#![allow(dead_code)]

use core::arch::asm;
use core::sync::atomic::{compiler_fence, Ordering};

use crate::config;

// -------------------------------------------------------------------------
// CSR accessors
// -------------------------------------------------------------------------

/// Read the `mhartid` CSR — current hart (core) ID.
///
/// C equivalent: `mempool_get_core_id()` in `runtime.h`
#[inline(always)]
pub fn core_id() -> u32 {
    let id: u32;
    unsafe {
        asm!("csrr {}, mhartid", out(reg) id, options(nomem, nostack));
    }
    id
}

/// Read the `mcycle` CSR — monotonic cycle counter.
///
/// C equivalent: `mempool_get_timer()` in `runtime.h`
#[inline(always)]
pub fn read_mcycle() -> u32 {
    let val: u32;
    unsafe {
        asm!("csrr {}, mcycle", out(reg) val, options(nomem, nostack));
    }
    val
}

/// Write 1 to the `trace` CSR (0x7D0) — start benchmark trace.
///
/// C equivalent: `mempool_start_benchmark()` in `runtime.h`
#[inline(always)]
pub fn start_benchmark() {
    compiler_fence(Ordering::SeqCst);
    unsafe {
        asm!("csrw 0x7D0, {}", in(reg) 1u32, options(nostack));
    }
    compiler_fence(Ordering::SeqCst);
}

/// Write 0 to the `trace` CSR (0x7D0) — stop benchmark trace.
///
/// C equivalent: `mempool_stop_benchmark()` in `runtime.h`
#[inline(always)]
pub fn stop_benchmark() {
    compiler_fence(Ordering::SeqCst);
    unsafe {
        asm!("csrw 0x7D0, {}", in(reg) 0u32, options(nostack));
    }
    compiler_fence(Ordering::SeqCst);
}

/// Single `nop` instruction — prevents spin-loop optimization.
#[inline(always)]
pub fn nop() {
    unsafe {
        asm!("nop", options(nomem, nostack));
    }
}

/// `wfi` instruction — wait for interrupt (low-power halt).
///
/// C equivalent: `mempool_wfi()` in `runtime.h`
#[inline(always)]
pub fn wfi() {
    unsafe {
        asm!("wfi", options(nomem, nostack));
    }
}

/// Busy-wait loop consuming approximately `cycles` iterations.
///
/// Uses the same asm template as the C runtime:
/// `addi counter, counter, -2; bgtz counter, loop`
///
/// C equivalent: `mempool_wait(uint32_t cycles)` in `runtime.h`
#[inline(always)]
pub fn wait(cycles: u32) {
    let counter = cycles;
    unsafe {
        asm!(
            "1:",
            "addi {0}, {0}, -2",
            "bgtz {0}, 1b",
            inlateout(reg) counter => _,
            options(nostack),
        );
    }
}

// -------------------------------------------------------------------------
// Topology queries — compile-time constants from build.rs config
// -------------------------------------------------------------------------

/// Number of cores in the cluster.
///
/// C: `mempool_get_core_count()` → `return NUM_CORES;`
#[inline(always)]
pub fn num_cores() -> u32 {
    config::NUM_CORES
}

/// Number of tiles in the cluster.
///
/// C: `mempool_get_tile_count()` → `return NUM_CORES / NUM_CORES_PER_TILE;`
#[inline(always)]
pub fn tile_count() -> u32 {
    config::NUM_CORES / config::NUM_CORES_PER_TILE
}

/// Tile ID of the current core.
///
/// C: `mempool_get_tile_id()` → `return core_id / NUM_CORES_PER_TILE;`
#[inline(always)]
pub fn tile_id() -> u32 {
    core_id() / config::NUM_CORES_PER_TILE
}

/// Number of groups in the cluster.
///
/// C: `mempool_get_group_count()` → `return NUM_GROUPS;`
#[inline(always)]
pub fn group_count() -> u32 {
    config::NUM_GROUPS
}

/// Group ID of the current core.
///
/// C: `mempool_get_group_id()` → `return core_id / (NUM_CORES / NUM_GROUPS);`
///
/// Returns 0 in degenerate configs where `NUM_CORES < NUM_GROUPS`.
#[inline(always)]
pub fn group_id() -> u32 {
    let cpg = cores_per_group();
    if cpg == 0 { 0 } else { core_id() / cpg }
}

/// Number of cores per tile.
///
/// C: `mempool_get_core_count_per_tile()` → `return NUM_CORES_PER_TILE;`
#[inline(always)]
pub fn cores_per_tile() -> u32 {
    config::NUM_CORES_PER_TILE
}

/// Number of cores per group.
///
/// C: `mempool_get_core_count_per_group()` → `return NUM_CORES / NUM_GROUPS;`
///
/// Returns 0 in degenerate configs where `NUM_CORES < NUM_GROUPS`.
#[inline(always)]
pub fn cores_per_group() -> u32 {
    if config::NUM_GROUPS == 0 { 0 } else { config::NUM_CORES / config::NUM_GROUPS }
}

// -------------------------------------------------------------------------
// Hardware-assisted polling — Zawrs + Zihintntl on shared-L1 TCDM
//
// On a 256-core MemPool cluster with a single shared L1 TCDM, naive
// spin-loops (lw + nop + branch) generate continuous load traffic on the
// crossbar for every polling iteration on every waiting core. With N
// cores spinning, this is O(N) sustained load requests per cycle —
// saturating bank arbiters and stalling productive data-plane traffic.
//
// The LR.W + WRS.NTO sequence eliminates polling traffic entirely:
// the hart suspends after establishing a reservation set, and only
// resumes when the producer's store invalidates the reserved cacheline.
// The NTL.PALL hint prevents the polled address from displacing
// application working-set data in any cache level.
// -------------------------------------------------------------------------

/// Hardware-assisted polling: spins until `*addr >= expected`.
///
/// Uses the RISC-V Zawrs + Zihintntl extensions to minimize crossbar
/// interconnect traffic on MemPool's shared-L1 TCDM:
///
/// 1. **NTL.PALL** (Zihintntl): Non-temporal locality hint — marks the
///    next memory access as ephemeral across all cache levels. On
///    MemPool, this prevents the polled cacheline from displacing
///    valuable application data in L1 bank buffers.
///
/// 2. **LR.W.AQ** (A extension): Load-Reserved Word with acquire
///    ordering. Reads the atomic variable and establishes a hardware
///    reservation set on the containing cacheline. The `.aq` suffix
///    enforces RVWMO acquire semantics: all subsequent loads/stores by
///    this hart are ordered after this load.
///
/// 3. **WRS.NTO** (Zawrs, opcode `0x00D00073`): Wait on Reservation
///    Set, No Timeout. If the loaded value does not satisfy the exit
///    condition, the hart enters a low-power suspended state. It
///    resumes only when:
///    - Another hart's store invalidates the reserved cacheline, OR
///    - An interrupt or other implementation-defined event occurs
///      (spurious wakeup — handled by the retry loop).
///
/// The loop structure handles spurious wakeups: after each resume, we
/// re-execute NTL.PALL + LR.W.AQ and re-check the condition.
///
/// # Emitted instruction sequence
///
/// ```text
/// 2:
///     add  x0, x0, x3          # NTL.PALL (Zihintntl)
///     lr.w.aq  val, (addr)     # load-reserved + acquire
///     bgeu val, expected, 3f   # exit if val >= expected
///     WRS.NTO                  # suspend until cacheline invalidated
///     j    2b                  # retry
/// 3:
/// ```
///
/// # Usage
///
/// Replaces `nop()` spin-loops in barrier and consumer paths:
///
/// ```rust,ignore
/// // Before (Phase 1):
/// while barrier.load(Ordering::SeqCst) < expected { nop(); }
///
/// // After (Phase 2):
/// hardware_spin_wait(&barrier, expected);
/// ```
///
/// Returns the loaded value (with acquire ordering) once `*addr >= expected`.
#[inline(always)]
pub fn hardware_spin_wait(addr: &core::sync::atomic::AtomicU32, expected: u32) -> u32 {
    let val: u32;
    let ptr = addr as *const core::sync::atomic::AtomicU32 as *const u32;
    unsafe {
        asm!(
            // Retry loop entry
            "2:",
            // ---- NTL.PALL (Zihintntl) ----
            // Encoded as R-type: add x0, x0, x3 (32-bit, never compressed)
            // Hint: next memory access has no temporal locality at any level
            ".insn r 0x33, 0, 0, x0, x0, x3",
            // ---- LR.W.AQ (A extension) ----
            // Load-reserved with acquire ordering (RVWMO)
            // Establishes reservation set on the cacheline containing *ptr
            "lr.w.aq {val}, ({ptr})",
            // ---- Condition check ----
            // Exit loop if loaded value >= expected (unsigned)
            "bgeu {val}, {exp}, 3f",
            // ---- WRS.NTO (Zawrs) ----
            // Wait on Reservation Set, No Timeout
            // Hart suspends until reservation is invalidated by a remote store
            // Opcode: 0x00D00073 (SYSTEM-type, imm=0x00D)
            ".insn i 0x73, 0, x0, x0, 0xD",
            // Retry after wakeup (may be spurious)
            "j 2b",
            "3:",
            ptr = in(reg) ptr,
            exp = in(reg) expected,
            val = out(reg) val,
            // No nomem — the asm block reads from memory via lr.w.
            // No pure — lr.w has side effects (reservation set).
            // The absence of nomem acts as a compiler fence, preventing
            // reordering of Rust loads/stores across this asm block.
            options(nostack),
        );
    }
    val
}
