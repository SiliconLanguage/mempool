//! FFI bindings to MemPool's `synchronization.h` barrier primitives.
//!
//! The C barrier functions are compiled from `synchronization.c` in the
//! MemPool runtime and linked as `.o` object files. These `extern "C"`
//! declarations resolve at link time against those objects.
//!
//! Safe wrappers are provided so the high-level channel API can
//! synchronize cores without writing `unsafe` blocks.

#![allow(dead_code)]

// -------------------------------------------------------------------------
// Raw FFI declarations
// -------------------------------------------------------------------------

extern "C" {
    fn mempool_barrier_init(core_id: u32);
    fn mempool_barrier(num_cores: u32);
    fn mempool_log_barrier(step: u32, core_id: u32);
    fn mempool_anyradixlog_barrier(radix: u32, core_id: u32);
    fn mempool_log_partial_barrier(step: u32, core_id: u32, num_cores_barrier: u32);
    fn mempool_linlog_barrier(step: u32, core_id: u32);
    fn mempool_partial_barrier(
        core_id: u32,
        core_init: u32,
        num_sleeping_cores: u32,
        memloc: u32,
    );
}

// -------------------------------------------------------------------------
// Safe wrappers
//
// The underlying C functions access shared L1 TCDM barrier state via
// atomic memory operations and are thread-safe by design — each core
// calls with its own `core_id` and the functions implement hardware-
// appropriate fence/AMO synchronization.
// -------------------------------------------------------------------------

/// Initialize per-core barrier state. Call once per core at startup.
///
/// C: `mempool_barrier_init(uint32_t core_id)` in `synchronization.h`
#[inline(always)]
pub fn barrier_init(core_id: u32) {
    unsafe { mempool_barrier_init(core_id) }
}

/// Full-cluster barrier — blocks until all `num_cores` cores arrive.
///
/// C: `mempool_barrier(uint32_t num_cores)` in `synchronization.h`
#[inline(always)]
pub fn barrier(num_cores: u32) {
    unsafe { mempool_barrier(num_cores) }
}

/// Logarithmic barrier with O(log N) synchronization latency.
///
/// C: `mempool_log_barrier(uint32_t step, uint32_t core_id)`
#[inline(always)]
pub fn log_barrier(step: u32, core_id: u32) {
    unsafe { mempool_log_barrier(step, core_id) }
}

/// Any-radix logarithmic barrier.
///
/// C: `mempool_anyradixlog_barrier(uint32_t radix, uint32_t core_id)`
#[inline(always)]
pub fn anyradixlog_barrier(radix: u32, core_id: u32) {
    unsafe { mempool_anyradixlog_barrier(radix, core_id) }
}

/// Partial logarithmic barrier for a subset of cores.
///
/// C: `mempool_log_partial_barrier(uint32_t step, uint32_t core_id,
///     uint32_t num_cores_barrier)`
#[inline(always)]
pub fn log_partial_barrier(step: u32, core_id: u32, num_cores_barrier: u32) {
    unsafe { mempool_log_partial_barrier(step, core_id, num_cores_barrier) }
}

/// Linear-logarithmic barrier.
///
/// C: `mempool_linlog_barrier(uint32_t step, uint32_t core_id)`
#[inline(always)]
pub fn linlog_barrier(step: u32, core_id: u32) {
    unsafe { mempool_linlog_barrier(step, core_id) }
}

/// Partial barrier with sleep-based waiting for core subsets.
///
/// C: `mempool_partial_barrier(uint32_t core_id, uint32_t core_init,
///     uint32_t num_sleeping_cores, uint32_t memloc)`
#[inline(always)]
pub fn partial_barrier(
    core_id: u32,
    core_init: u32,
    num_sleeping_cores: u32,
    memloc: u32,
) {
    unsafe { mempool_partial_barrier(core_id, core_init, num_sleeping_cores, memloc) }
}
