//! MemPool-IPC Phase 1: Native Rust Wrapper
//!
//! `#![no_std]` bare-metal crate providing:
//! - Rust-native SPSC lock-free queue over MemPool's L1 TCDM
//! - Safe wrappers for the MemPool C runtime (barriers, allocator, MMIO)
//! - RVWMO acquire/release atomics
//! - Conditionally compiled hardware-assisted polling (Zawrs / fallback)
//!
//! Entry point (`extern "C" fn main`) is in `src/main.rs`, included as
//! a submodule. Linked with MemPool's C runtime (`crt0.S`).

#![no_std]
#![no_main]

// -------------------------------------------------------------------------
// Modules
// -------------------------------------------------------------------------

#[allow(dead_code)]
mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

/// Bare-metal RISC-V primitives: CSR access, topology queries, timing,
/// and the conditionally compiled `hardware_spin_wait`.
pub mod primitives;

/// MMIO hardware register map: wake-up, EOC, cache control.
pub mod hw;

/// FFI bindings to MemPool synchronization barrier functions.
pub mod sync;

/// FFI bindings to MemPool dynamic memory allocator.
pub mod alloc_ffi;

/// Bare-metal entry point (extern "C" fn main) and SPSC queue test.
#[path = "main.rs"]
mod entry;
