//! MemPool hardware register map (MMIO).
//!
//! Translates `addrmap.h` + `control_registers.h` into typed Rust constants
//! and provides safe wrappers for volatile MMIO register writes (wake-up,
//! end-of-computation, cache control).
//!
//! The MMIO addresses are hardware-defined constants that are always valid on
//! MemPool targets. Writes have side effects (waking cores, signalling EOC)
//! but cannot cause memory unsafety — a volatile write to a fixed hardware
//! register is not UB on the intended target.

#![allow(dead_code)]

use core::ptr;

// -------------------------------------------------------------------------
// MMIO base address — `CONTROL_REGISTER_OFFSET` in `addrmap.h`
// -------------------------------------------------------------------------

pub const CONTROL_BASE: usize = 0x4000_0000;

// -------------------------------------------------------------------------
// Register offsets — from `control_registers.h`
// -------------------------------------------------------------------------

/// End-of-Computation register.
pub const EOC_OFFSET: usize = 0x00;
/// Wake Up register (single core or broadcast).
pub const WAKE_UP_OFFSET: usize = 0x04;
/// Wake Up Tile registers — one per group (0..7).
pub const WAKE_UP_TILE_OFFSETS: [usize; 8] = [
    0x08, 0x0C, 0x10, 0x14, 0x18, 0x1C, 0x20, 0x24,
];
/// Wake Up Group register (group-level mask).
pub const WAKE_UP_GROUP_OFFSET: usize = 0x28;
/// Wake Up Stride register.
pub const WAKE_UP_STRIDE_OFFSET: usize = 0x2C;
/// Wake Up Offset register.
pub const WAKE_UP_OFFST_OFFSET: usize = 0x30;
/// TCDM Start Address register.
pub const TCDM_START_OFFSET: usize = 0x34;
/// TCDM End Address register.
pub const TCDM_END_OFFSET: usize = 0x38;
/// Number of Cores register.
pub const NR_CORES_OFFSET: usize = 0x3C;
/// Read-only cache Enable register.
pub const RO_CACHE_ENABLE_OFFSET: usize = 0x40;
/// Read-only cache Flush register.
pub const RO_CACHE_FLUSH_OFFSET: usize = 0x44;
/// Read-only cache Region Start registers (0..3).
pub const RO_CACHE_START_OFFSETS: [usize; 4] = [0x48, 0x4C, 0x50, 0x54];
/// Read-only cache Region End registers (0..3).
pub const RO_CACHE_END_OFFSETS: [usize; 4] = [0x58, 0x5C, 0x60, 0x64];

// -------------------------------------------------------------------------
// MMIO volatile helpers
// -------------------------------------------------------------------------

/// Volatile write to a 32-bit MMIO register.
///
/// # Safety
/// `addr` must be a valid, aligned MMIO address on the target hardware.
#[inline(always)]
unsafe fn mmio_write(addr: usize, val: u32) {
    ptr::write_volatile(addr as *mut u32, val);
}

/// Volatile read from a 32-bit MMIO register.
///
/// # Safety
/// `addr` must be a valid, aligned MMIO address on the target hardware.
#[inline(always)]
unsafe fn mmio_read(addr: usize) -> u32 {
    ptr::read_volatile(addr as *const u32)
}

// -------------------------------------------------------------------------
// Wake-up API — safe wrappers around C `wake_up*()` from `runtime.h`
//
// The C runtime defines these as `static inline` functions that do a
// single volatile store to a fixed MMIO address. We replicate the same
// volatile write pattern.
// -------------------------------------------------------------------------

/// Wake up a specific core by ID, or all cores if `core_id == u32::MAX`.
///
/// C: `wake_up(uint32_t core_id)` in `runtime.h`
#[inline(always)]
pub fn wake_up(core_id: u32) {
    unsafe { mmio_write(CONTROL_BASE + WAKE_UP_OFFSET, core_id) }
}

/// Wake up all cores (broadcast).
///
/// C: `wake_up_all()` → `wake_up((uint32_t)-1)`
#[inline(always)]
pub fn wake_up_all() {
    wake_up(u32::MAX);
}

/// Wake up cores by group bitmask.
///
/// C: `wake_up_group(uint32_t group_mask)`
#[inline(always)]
pub fn wake_up_group(group_mask: u32) {
    unsafe { mmio_write(CONTROL_BASE + WAKE_UP_GROUP_OFFSET, group_mask) }
}

/// Wake up all groups.
///
/// C: `wake_up_all_group()` → `wake_up_group((uint32_t)-1)`
#[inline(always)]
pub fn wake_up_all_groups() {
    wake_up_group(u32::MAX);
}

/// Wake up tiles within a specific group by tile bitmask.
///
/// Falls back to group 0 if `group_id >= 8` (matches C `default` case).
///
/// C: `wake_up_tile(uint32_t group_id, uint32_t tile_mask)`
#[inline(always)]
pub fn wake_up_tile(group_id: u32, tile_mask: u32) {
    let idx = if group_id < 8 { group_id as usize } else { 0 };
    unsafe { mmio_write(CONTROL_BASE + WAKE_UP_TILE_OFFSETS[idx], tile_mask) }
}

/// Set the wake-up stride register.
///
/// C: `set_wake_up_stride(uint32_t stride)`
#[inline(always)]
pub fn set_wake_up_stride(stride: u32) {
    unsafe { mmio_write(CONTROL_BASE + WAKE_UP_STRIDE_OFFSET, stride) }
}

/// Set the wake-up offset register.
///
/// C: `set_wake_up_offset(uint32_t offset)`
#[inline(always)]
pub fn set_wake_up_offset(offset: u32) {
    unsafe { mmio_write(CONTROL_BASE + WAKE_UP_OFFST_OFFSET, offset) }
}

// -------------------------------------------------------------------------
// Miscellaneous control registers
// -------------------------------------------------------------------------

/// Signal end-of-computation to the testbench / host.
#[inline(always)]
pub fn signal_eoc(val: u32) {
    unsafe { mmio_write(CONTROL_BASE + EOC_OFFSET, val) }
}

/// Read the TCDM start address from the hardware register.
#[inline(always)]
pub fn tcdm_start() -> u32 {
    unsafe { mmio_read(CONTROL_BASE + TCDM_START_OFFSET) }
}

/// Read the TCDM end address from the hardware register.
#[inline(always)]
pub fn tcdm_end() -> u32 {
    unsafe { mmio_read(CONTROL_BASE + TCDM_END_OFFSET) }
}

/// Read the number of cores from the hardware register.
#[inline(always)]
pub fn hw_num_cores() -> u32 {
    unsafe { mmio_read(CONTROL_BASE + NR_CORES_OFFSET) }
}

/// Enable/disable the read-only cache.
#[inline(always)]
pub fn ro_cache_enable(enable: u32) {
    unsafe { mmio_write(CONTROL_BASE + RO_CACHE_ENABLE_OFFSET, enable) }
}

/// Flush the read-only cache.
#[inline(always)]
pub fn ro_cache_flush() {
    unsafe { mmio_write(CONTROL_BASE + RO_CACHE_FLUSH_OFFSET, 1) }
}

/// Set a read-only cache region's start address (region 0..3).
#[inline(always)]
pub fn ro_cache_set_start(region: usize, addr: u32) {
    if region < 4 {
        unsafe { mmio_write(CONTROL_BASE + RO_CACHE_START_OFFSETS[region], addr) }
    }
}

/// Set a read-only cache region's end address (region 0..3).
#[inline(always)]
pub fn ro_cache_set_end(region: usize, addr: u32) {
    if region < 4 {
        unsafe { mmio_write(CONTROL_BASE + RO_CACHE_END_OFFSETS[region], addr) }
    }
}
