#![no_std]

//! # MemPool-IPC: Phase 1 - Native Wrapper
//!
//! This crate provides the foundational Unsafe/FFI layer for bridging the 
//! bare-metal MemPool C runtime and the Rust framework.

/// Hardware-assisted IPC queue attributes.
pub mod mmio {
    use core::ptr::{read_volatile, write_volatile};

    /// Performs a volatile write to a TCDM memory address.
    ///
    /// # Safety
    /// The caller must ensure the pointer is aligned and points to valid TCDM memory.
    pub unsafe fn write_tcdm(addr: *mut u32, val: u32) {
        // Rust 2024 requires explicit unsafe blocks even inside unsafe functions
        unsafe {
            write_volatile(addr, val);
        }
    }

    /// Performs a volatile read from a TCDM memory address.
    ///
    /// # Safety
    /// The caller must ensure the pointer is aligned and points to valid TCDM memory.
    pub unsafe fn read_tcdm(addr: *const u32) -> u32 {
        unsafe {
            read_volatile(addr)
        }
    }
}

/// Foreign Function Interface (FFI) bindings to the MemPool C Runtime.
pub mod ffi {
    // Recent Rust editions require extern blocks themselves to be marked unsafe
    unsafe extern "C" {
        // Symbols will be added here as we map the C headers
        // pub fn mempool_get_core_id() -> u32;
    }
}

#[cfg(test)]
mod tests {
    // Simulation-based tests in Banshee
}