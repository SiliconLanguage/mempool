// L1 TCDM globals for Phase 1 Rust — provides symbols consumed by lib.rs
// These are placed in the .l1 (NOLOAD) section, residing in L1 TCDM at runtime.

.section .l1, "aw", @nobits

// HardwareQueue: 65664 bytes (0x10080), aligned to 64
.globl ipc_queue
.align 6
ipc_queue:
    .space 0x10080

// Boot barrier: 4 bytes, aligned to 64 (separate cacheline)
.globl boot_barrier
.align 6
boot_barrier:
    .space 4
