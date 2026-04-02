# MemPool-IPC: Hardware-Assisted Monadic Messaging Framework
**Engineered by [SiliconLanguage](https://siliconlanguage.com/)**

MemPool-IPC is a bare-metal, zero-copy messaging framework and data plane specifically architected for RISC-V many-core scaled-up clusters, such as MemPool (256 cores, 1 MiB shared L1) and TeraPool (1024 cores, 4 MiB shared L1) [1, 2]. 

By pushing synchronization primitives down to the microarchitecture layer, MemPool-IPC bypasses traditional software-managed locks, OS kernel involvement, and cache-coherency bottlenecks to enable line-rate tensor streaming for distributed AI workloads.

---

## 🔬 Core Architectural Innovations

### 1. Lock-Free RVWMO Atomics & False-Sharing Prevention
The framework implements strict Single-Producer Single-Consumer (SPSC) and Multi-Producer Multi-Consumer (MPMC) lock-free ring buffers. Queue slots are explicitly 64-byte aligned to map directly to single cache lines within the Tightly Coupled Data Memory (TCDM), eliminating interconnect congestion caused by "false sharing" [3]. Synchronization is guaranteed using the RISC-V Weak Memory Ordering (RVWMO) model, explicitly leveraging `.aq` (acquire) and `.rl` (release) semantics for multi-copy atomicity [4, 5].

### 2. Energy-Efficient Polling via `Zawrs`
Traditional spin-loops waste instruction-fetch bandwidth and generate heavy interconnect snooping traffic [6]. MemPool-IPC utilizes the RISC-V `Zawrs` (Wait-on-Reservation-Set) extension. Consumer harts establish a reservation set on a queue slot using a Load-Reserved (`LR`) instruction and then execute `WRS.NTO` (Wait-on-Reservation-Set, No Timeout) [7]. This forces the core into a low-power standby state until a producer's store invalidates the reservation, providing microsecond-level wake-up latency with near-zero idle power consumption [8, 9].

### 3. Cache Pollution Mitigation via `Zihintntl` & `Zicbom`
Streaming massive AI tensor payloads through producer-consumer queues typically displaces valuable long-lived data (like instruction caches) [10]. MemPool-IPC mitigates this by pairing Non-Temporal Locality hints (`NTL.PALL` from the `Zihintntl` extension) with explicit Cache-Block Management operations (`CBO.FLUSH` and `CBO.ZERO` from the `Zicbom` extension) [11, 12]. This commands the microarchitecture to stream payloads directly into the shared TCDM without polluting the local L0/L1 caches [12].

### 4. Strict Ordering via Physical Memory Attributes (PMAs)
To prevent speculative execution engines from prematurely "popping" data off the hardware queues, the framework mandates that queue memory regions be hardware-configured with specific Physical Memory Attributes (PMAs). Channels are mapped as strictly ordered and *non-idempotent*, ensuring absolute predictability in the data plane [13, 14].

---

## 📂 Project Structure

MemPool-IPC utilizes a phased, "hardware-first" implementation strategy, establishing ground-truth memory layouts in C before lifting them into idiomatic `no_std` Rust abstractions.

```text
mempool-ipc/
├── docs/                                 # Formal Architectural Specifications
│   ├── architecture-spec/                # Micro-Architecture & RVWMO Protocol
│   │   └── README.md                     
│   └── topology-aware-routing/           # TeraPool NUMA & Macro-Architecture
│       └── README.md                     
│
├── phase0_c/                             # Phase 0: Ground-Truth C-Subset
│   ├── include/queue.h                   # 64-byte aligned queue & __atomic_store_n
│   ├── src/main.c                        # SPMD hardware-in-the-loop test harness
│   └── Makefile                          # Hooks for Banshee emulator / RTL simulation
│
├── phase1_rust/                          # Phase 1–4: Idiomatic Rust Data Plane
│   ├── src/
│   │   ├── main.rs                      # Bare-metal entry point + HTIF exit
│   │   ├── lib.rs                       # High-level Sender/Receiver Channel API
│   │   └── primitives.rs               # Inline asm! for Zawrs & Zihintntl
│   ├── Makefile                         # Spike cross-compile & run pipeline
│   ├── build.rs                         # bindgen FFI configuration
│   └── Cargo.toml                       # no_std environment definitions
│
└── dataplane-emu/                        # Phase 5: Hardware Offloading & Zero-Copy I/O
    ├── include/dataplane_emu/
    │   ├── sl_intrinsics.h              # Unified RISC-V / ARM64 inline asm
    │   └── spsc_queue.h                 # Lock-free queue with hw-assisted polling
    ├── src/
    │   └── main.cpp                     # Host-mode functional verification
    └── Makefile                         # Host / cross-compile targets
```
---

## 🚀 Phase 5: Hardware Offloading & Zero-Copy I/O (`dataplane-emu/`)

Phase 5 introduces a **unified C++ hardware abstraction layer** that maps identical SPSC queue semantics onto both RISC-V and ARM64 DPU targets (e.g., NVIDIA BlueField-3).

### Unified Intrinsics — `sl_intrinsics.h`

The `sl_intrinsics.h` header provides two primitives that eliminate the "synchronization tax" across architectures:

| Primitive | RISC-V (Zawrs + Zihintntl) | ARM64 (ARMv8.1) | Purpose |
|-----------|---------------------------|-----------------|---------|
| `sl_wait_on_address()` | `LR.D.AQ` + `WRS.NTO` | `LDAXR` + `WFE` | Zero-traffic consumer polling — core suspends until producer's store invalidates reservation |
| `sl_load_nt()` | `NTL.PALL` + `LD` | `LDNP` (Load Non-Temporal Pair) | Cache pollution prevention — streaming data bypasses L1/L2 without displacing hot working set |

### Architectural Mapping

```
┌─────────────────────────────────────────────────────────────────┐
│                    sl_wait_on_address()                         │
├───────────────────────────┬─────────────────────────────────────┤
│ RISC-V (MemPool/TeraPool) │ ARM64 (BlueField-3 DPU)            │
│                           │                                     │
│ 1: LR.D.AQ  t0, (a0)    │ 1: LDAXR  x0, [x1]                 │
│    BEQ  t0, a1, 2f       │    CMP    x0, x2                    │
│    WRS.NTO                │    B.EQ   2f                        │
│    J    1b                │    WFE                              │
│ 2:                        │    B      1b                        │
│                           │ 2:                                  │
├───────────────────────────┼─────────────────────────────────────┤
│ Acquire: LR.D.AQ          │ Acquire: LDAXR (Load-Acquire Excl.) │
│ Suspend: WRS.NTO           │ Suspend: WFE (Wait For Event)       │
│ Wake: store invalidates    │ Wake: global monitor SEV             │
│        reservation set     │        (implicit on exclusive store) │
└───────────────────────────┴─────────────────────────────────────┘
```

### SmartNIC Deployment Path

The unified header enables a single C++ data plane codebase to target:
- **MemPool/TeraPool** — 256/1024-core RISC-V clusters with shared L1 TCDM
- **NVIDIA BlueField-3** — ARM64 Cortex-A78 cores with ConnectX-7 NIC offload
- **x86_64 hosts** — Functional verification (intrinsics compile to no-ops, queue uses `std::atomic`)

--------------------------------------------------------------------------------
📖 **Publications & Further Reading**

**[Architectural Analysis and Technical Specification for MemPool-IPC:](docs/architecture-spec/)** A formal analysis of hardware-assisted Rust messaging frameworks for RISC-V many-core architectures.

**[SiliconLanguage Foundry:](https://siliconlanguage.com/)** For additional research on the Monadic Cloud Hypervisor, user-space storage engines, and Software-Hardware Co-Design.

**Primary Author & Architect:** Ping Long, Chief Systems Architect | Founder, SiliconLanguage ping.long@siliconlanguage.com

***
