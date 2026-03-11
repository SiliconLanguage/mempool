# MemPool-IPC: Phase 1 (Native Wrapper)

A high-performance, Rust-based Inter-Process Communication (IPC) framework designed for the [MemPool (Pinwheel)](https://github.com/ping-long-github/mempool) many-core RISC-V architecture.

## 1. Project Overview
This framework provides a safe, concurrent abstraction over the MemPool Tightly Coupled Data Memory (TCDM). Phase 1 implements the **Native Wrapper**, establishing the critical Unsafe/FFI layer between the bare-metal C runtime and Rust.

### Key Architectural Constraints
- **Environment:** Strict bare-metal `no_std` execution.
- **Memory Topology:** Shared L1 TCDM (global view for up to 1024 cores).
- **Memory Attributes:** Non-idempotent and non-cacheable mapping for hardware-assisted IPC queues.

## 2. Features (Phase 1)
- **Volatile MMIO Primitives:** Verified `read_volatile` and `write_volatile` wrappers to prevent compiler elision of TCDM accesses.
- **FFI Foundation:** `unsafe extern "C"` blocks for mapping the MemPool runtime C headers (e.g., `mempool_get_core_id`).
- **Rust 2024 Standards:** Implementation utilizes explicit safety markers and internal unsafe blocks for maximum auditability.

## 3. Development & Build
Built using the standard Rust toolchain targeting RISC-V:

```bash
# Build the library
cargo build --target riscv32imac-unknown-none-elf