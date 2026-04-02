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
