//! FFI bindings to MemPool's `alloc.h` dynamic memory allocator.
//!
//! Provides access to the MemPool runtime's linked-list first-fit allocator
//! for L1 interleaved and L1 tile-local sequential memory regions.
//!
//! The C allocator is compiled from `alloc.c` in the MemPool runtime.
//! These FFI declarations resolve at link time.

#![allow(dead_code)]

use core::ffi::c_void;

// -------------------------------------------------------------------------
// C struct definitions — layout-compatible with `alloc.h`
// -------------------------------------------------------------------------

/// Free memory block node in the allocator's linked list.
///
/// C: `alloc_block_t` in `alloc.h`
#[repr(C)]
pub struct AllocBlock {
    pub size: u32,
    pub next: *mut AllocBlock,
}

/// Allocator state — head pointer into the free-block linked list.
///
/// C: `alloc_t` in `alloc.h`
#[repr(C)]
pub struct Alloc {
    pub first_block: *mut AllocBlock,
}

// -------------------------------------------------------------------------
// Raw FFI declarations
// -------------------------------------------------------------------------

extern "C" {
    fn alloc_init(alloc: *mut Alloc, base: *mut c_void, size: u32);
    fn simple_malloc(size: u32) -> *mut c_void;
    fn domain_malloc(alloc: *mut Alloc, size: u32) -> *mut c_void;
    fn simple_free(ptr: *mut c_void);
    fn domain_free(alloc: *mut Alloc, ptr: *mut c_void);
    fn alloc_dump(alloc: *mut Alloc);
    fn get_alloc_l1() -> *mut Alloc;
    fn get_alloc_tile(tile_id: u32) -> *mut Alloc;
}

// -------------------------------------------------------------------------
// Safe wrappers
//
// Allocation returns raw pointers (possibly null). The caller must
// manage pointer validity and lifetimes. Free functions are marked
// `unsafe` because passing an invalid pointer causes heap corruption.
// -------------------------------------------------------------------------

/// Initialize an allocator over a memory region.
///
/// C: `alloc_init(alloc_t *alloc, void *base, uint32_t size)`
///
/// # Safety
/// `alloc` must point to a valid `Alloc` struct. `base` must point to
/// the start of an allocatable memory region of at least `size` bytes.
pub unsafe fn init_allocator(alloc: *mut Alloc, base: *mut u8, size: u32) {
    alloc_init(alloc, base as *mut c_void, size)
}

/// Allocate `size` bytes from the L1 interleaved heap.
/// Returns a null pointer on failure.
///
/// C: `simple_malloc(uint32_t size)`
pub fn l1_malloc(size: u32) -> *mut u8 {
    unsafe { simple_malloc(size) as *mut u8 }
}

/// Allocate `size` bytes from a specific domain allocator.
/// Returns a null pointer on failure.
///
/// C: `domain_malloc(alloc_t *alloc, uint32_t size)`
pub fn domain_alloc(alloc: *mut Alloc, size: u32) -> *mut u8 {
    unsafe { domain_malloc(alloc, size) as *mut u8 }
}

/// Free a pointer previously returned by [`l1_malloc`].
///
/// C: `simple_free(void *ptr)`
///
/// # Safety
/// `ptr` must have been returned by `l1_malloc` and not yet freed.
pub unsafe fn l1_free(ptr: *mut u8) {
    simple_free(ptr as *mut c_void)
}

/// Free a pointer previously returned by [`domain_alloc`].
///
/// C: `domain_free(alloc_t *alloc, void *ptr)`
///
/// # Safety
/// `ptr` must have been returned by `domain_alloc` on the same
/// allocator and not yet freed.
pub unsafe fn domain_dealloc(alloc: *mut Alloc, ptr: *mut u8) {
    domain_free(alloc, ptr as *mut c_void)
}

/// Dump the allocator's free-block list (debug, calls C `printf`).
///
/// C: `alloc_dump(alloc_t *alloc)`
pub fn dump_allocator(alloc: *mut Alloc) {
    unsafe { alloc_dump(alloc) }
}

/// Get the L1 interleaved heap allocator.
///
/// C: `get_alloc_l1()`
pub fn l1_allocator() -> *mut Alloc {
    unsafe { get_alloc_l1() }
}

/// Get the tile-local sequential heap allocator for a given tile.
///
/// C: `get_alloc_tile(uint32_t tile_id)`
pub fn tile_allocator(tile_id: u32) -> *mut Alloc {
    unsafe { get_alloc_tile(tile_id) }
}
