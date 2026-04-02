# MemPool-IPC Phase 0: C-Subset Ground Truth

Bare-metal SPSC (Single-Producer, Single-Consumer) lock-free queue on
the MemPool RISC-V many-core processor. This is the **ground truth C
implementation** that validates RVWMO acquire/release atomics on shared
L1 TCDM before porting to higher-level languages.

## Directory Layout

```
phase0_c/
├── Makefile              # Build system (256-core main + 2-hart Spike targets)
├── src/
│   └── main.c            # SPSC queue test: producer/consumer on two cores
├── include/
│   └── queue.h           # Lock-free ring buffer with RVWMO atomics
├── spike_htif.S          # HTIF tohost/fromhost symbols for Spike emulator
├── spike_link.ld         # Spike-specific linker script (includes spike_arch.ld)
└── banshee-src/          # (future) Banshee emulator sources
```

**Runtime dependencies** (not in this directory):
```
../../runtime/
├── crt0.S                # Boot code: register init, stack setup, jump to main()
├── arch.ld.c             # Linker memory map template (CPP-expanded)
├── link.ld               # Section layout (L1 NOLOAD, L2 text/data)
├── runtime.h             # mempool_get_core_id(), mempool_get_core_count(), etc.
└── addrmap.h             # Control register offsets
```

## Prerequisites

### Toolchain

The RISC-V bare-metal GCC cross-compiler must be on your `PATH`:

```bash
export PATH=/home/dragonix/mempool/install/riscv-gcc/bin:$PATH
```

Verify:
```bash
riscv64-unknown-elf-gcc --version
# Expected: riscv64-unknown-elf-gcc (gc891d8dc23e) 13.2.0
```

### Spike Emulator (for `run-spike`)

The custom MemPool Spike build is required:
```bash
ls /home/dragonix/mempool/install/riscv-isa-sim/bin/spike
# Should exist. If not, build it: cd toolchain/riscv-isa-sim && mkdir build && cd build && ../configure --prefix=... && make install
```

## Building

All commands assume you are in the `phase0_c/` directory:
```bash
cd /home/dragonix/mempool/software/mempool-ipc/phase0_c
```

### Main Build (256-core MemPool ELF)

```bash
make all
```

**Produces:**
| Artifact | Description |
|----------|-------------|
| `arch.ld` | Generated linker memory map (256 cores × 4 banks × 1KB = 1MB L1) |
| `mempool_ipc_phase0.elf` | 32-bit RISC-V ELF, entry at `0x80000000` |
| `mempool_ipc_phase0.dump` | Full disassembly for RVWMO atomic verification |

This ELF targets the full 256-core MemPool hardware or
cycle-accurate RTL simulation. It is **not** runnable on Spike (core
count mismatch, missing control registers).

### Spike Build (2-hart functional validation)

```bash
make run-spike
```

This single command compiles, links, strips, and runs the ELF on
Spike. It automatically:

1. Recompiles `main.c` and `crt0.S` with `NUM_CORES=2`
2. Generates `spike_arch.ld` with L1 ORIGIN relocated to `0x10000`
3. Links with `spike_link.ld` (includes `spike_arch.ld` instead of `arch.ld`)
4. Strips `.l1_seq` and `.l1` NOLOAD sections (Spike's ELF loader cannot handle them)
5. Runs Spike with a 10-second timeout

**Build-only** (no execution):
```bash
make mempool_ipc_phase0_spike_stripped.elf
```

### Clean

```bash
make clean         # Remove main build artifacts
make clean-spike   # Remove Spike build artifacts
```

## Running on Spike

### Default (10-second timeout)

```bash
make run-spike
```

The program will loop indefinitely because MemPool's `_eoc` handler
writes to a control register at `0x40000000` then returns to the boot
ROM — there is no HTIF `ecall` termination. The `timeout 10` wrapper
kills Spike after 10 seconds. The `make` error 124 from timeout is
intentionally ignored (prefixed with `-`).

### Manual Spike Invocation

```bash
/home/dragonix/mempool/install/riscv-isa-sim/bin/spike \
  --isa=rv32ima \
  -p2 \
  -m0x10000:0x40000,0x40000000:0x100000,0x80000000:0x400000 \
  mempool_ipc_phase0_spike_stripped.elf
```

Press `Ctrl-C` to stop.

### Spike with Instruction Trace (Debugging)

```bash
/home/dragonix/mempool/install/riscv-isa-sim/bin/spike \
  --isa=rv32ima \
  -p2 \
  -l \
  -m0x10000:0x40000,0x40000000:0x100000,0x80000000:0x400000 \
  mempool_ipc_phase0_spike_stripped.elf \
  2>/tmp/spike_trace.log &

# Wait a few seconds then kill
sleep 5 && kill %1

# Inspect the trace
wc -l /tmp/spike_trace.log          # Expect ~7-8M lines for 5 seconds
grep "exception" /tmp/spike_trace.log  # Should return 0 results = no faults
```

### Interactive Spike Debugger

```bash
/home/dragonix/mempool/install/riscv-isa-sim/bin/spike \
  --isa=rv32ima \
  -p2 \
  -d \
  -m0x10000:0x40000,0x40000000:0x100000,0x80000000:0x400000 \
  mempool_ipc_phase0_spike_stripped.elf
```

Useful debug commands inside Spike's interactive console:
```
: reg 0                    # Show all registers for hart 0
: reg 0 a0                 # Show register a0 on hart 0
: pc 0                     # Show PC for hart 0
: pc 1                     # Show PC for hart 1
: mem 0 0x21000            # Read memory at boot_barrier (L1)
: mem 0 0x21040            # Read memory at ipc_queue base (L1)
: until pc 0 0x80000104    # Run hart 0 until it reaches main()
: run 1000                 # Execute 1000 instructions
: quit                     # Exit
```

## Memory Map

### Main Build (256-core)

| Region | Start | End | Size | Contents |
|--------|-------|-----|------|----------|
| L1 TCDM | `0x00000000` | `0x00100000` | 1 MB | Stacks, sequential region, queue, barrier |
| Control Regs | `0x40000000` | `0x40100000` | 1 MB | Wake-up, EOC, RO cache config |
| L2 SPM | `0x80000000` | `0x80400000` | 4 MB | Code (`.text`), read-only data, `.data` |
| Boot ROM | `0x00001000` | `0x00002000` | 4 KB | Reset vector (hardware) |

### Spike Build (2-hart)

| Region | Start | End | Size | Contents |
|--------|-------|-----|------|----------|
| L1 TCDM | `0x00010000` | `0x00050000` | 256 KB | Stacks, seq region, queue, barrier |
| Control Regs | `0x40000000` | `0x40100000` | 1 MB | (mapped but not functional in Spike) |
| L2 SPM | `0x80000000` | `0x80400000` | 4 MB | Code, data, HTIF symbols |
| Spike Boot ROM | `0x00001000` | — | Internal | Spike's built-in reset vector |

**Why L1 starts at `0x10000` on Spike:** Spike places its internal
boot ROM at `0x1000` (DEFAULT_RSTVEC). If L1 ORIGIN were `0x0`,
`boot_barrier` and `ipc_queue` would overlap the ROM, causing
`trap_store_access_fault`.

## Key Symbols (Spike ELF)

| Symbol | Address | Description |
|--------|---------|-------------|
| `_start` | `0x80000000` | Entry point (crt0.S boot code) |
| `main` | `0x80000104` | C main function |
| `_eoc` | `0x800000e8` | End-of-computation handler |
| `boot_barrier` | `0x00021000` | Atomic barrier counter (L1) |
| `ipc_queue` | `0x00021040` | HardwareQueue struct base (L1) |
| `tohost` | `0x80001000` | HTIF host communication (L2) |
| `fromhost` | `0x80001008` | HTIF device communication (L2) |
| `__stack_start` | `0x00010000` | Stack base for both harts |
| `__l1_end` | `0x00050000` | End of L1 region |

## How the Code Works

### Boot Sequence (`crt0.S`)

1. Initialize global pointer (`gp`)
2. Zero all general-purpose registers (`x1`–`x31`)
3. Compute per-hart stack pointer from `mhartid`, tile geometry, and `STACK_SIZE`
4. Hart 0 configures the RO cache end register at `0x40000058`
5. All harts call `main()`

### Main Logic (`src/main.c`)

```
Hart 0 (Producer)              Hart 1 (Consumer)
─────────────────              ─────────────────
Init queue (1024 slots)
  ↓
hw_barrier(2) ←─────────────→ hw_barrier(2)
  ↓                              ↓
hw_wait(100 nops)              Enter consume_message() spin loop
  ↓                              ↓
publish_message(0xDEADBEEF)    Acquire-load sees sequence ≥ 1
  ↓                              ↓
hw_barrier(2) ←─────────────→ Read data (0xDEADBEEF), hw_barrier(2)
  ↓                              ↓
return 0 → _eoc               return 0 → _eoc
```

### RVWMO Atomics (`include/queue.h`)

The queue uses RISC-V Weak Memory Ordering (RVWMO) primitives:

- **`publish_message()`**: Writes data, then `__atomic_store_n(..., __ATOMIC_RELEASE)`
  → compiles to `fence iorw,ow` + `amoswap.w` (release store)
- **`consume_message()`**: `__atomic_load_n(..., __ATOMIC_ACQUIRE)`
  → compiles to `fence` + load (acquire load)
- **`hw_barrier()`**: `__atomic_fetch_add(..., __ATOMIC_SEQ_CST)`
  → compiles to `fence iorw,ow` + `amoadd.w.aq` (sequentially consistent RMW)

These ensure the data write in the producer is visible to the consumer
before the sequence counter update, preventing torn reads on the
MemPool L1 TCDM interconnect.

### Ring Buffer Design

```
RING_SIZE = 1024 slots
Each QueueSlot = 64 bytes (cacheline-aligned to prevent false sharing)
  ├── sequence (uint32_t) — monotonically increasing counter
  └── data (uint32_t)     — payload (0xDEADBEEF in test)

Total HardwareQueue ≈ 65 KB (1024 × 64B slots + 2 × 64B aligned counters)
```

Index calculation: `slot = seq & (RING_SIZE - 1)` (power-of-2 masking).

## Debugging Guide

### Build Fails: "undefined symbol `L1_BANK_SIZE`" or "`BOOT_ADDR`"

The linker script template `arch.ld.c` requires these macros. They are
defined in `CFLAGS`. If you modify the Makefile, ensure these are present:
```
-DBOOT_ADDR=0x00001000 -DL1_BANK_SIZE=1024
```

### Build Fails: ".l1_seq section overflow"

The sequential memory region `NUM_CORES × SEQ_MEM_SIZE` must fit in L1
(`NUM_CORES × BANKING_FACTOR × L1_BANK_SIZE`). With 256 cores:
- L1 = 256 × 4 × 1024 = 1 MB
- Seq = 256 × SEQ_MEM_SIZE

If `SEQ_MEM_SIZE=8192`: 256 × 8192 = 2 MB > 1 MB → overflow.
Current fix: `SEQ_MEM_SIZE=2048` (256 × 2048 = 512 KB < 1 MB).

### Spike: `trap_store_access_fault` at boot

**Symptom:** Spike crashes immediately with a store access fault.

**Common causes:**
1. **L1 at address 0x0:** Spike's ELF loader uses `debug_mmu->store_uint64()`
   which faults on address 0. The stripped ELF (`_stripped.elf`) must be used.
2. **L1 overlaps Spike ROM at 0x1000:** The `spike_arch.ld` relocates L1
   ORIGIN to `0x10000` via `sed`. If this didn't run, `boot_barrier` lands
   on the ROM.
3. **Memory regions not declared:** Spike's `-m` flag must cover all
   accessed addresses. Current layout:
   ```
   -m0x10000:0x40000,0x40000000:0x100000,0x80000000:0x400000
   ```

**Debug steps:**
```bash
# Run with trace to find faulting address
spike --isa=rv32ima -p2 -l \
  -m0x10000:0x40000,0x40000000:0x100000,0x80000000:0x400000 \
  mempool_ipc_phase0_spike_stripped.elf 2>&1 | grep -i "exception\|trap" | head -5
```

### Spike: Program hangs / infinite loop

**Expected behavior.** The MemPool `_eoc` handler writes to the EOC
control register at `0x40000000` then jumps to `__rom_start` (boot
ROM), which restarts execution. There is no HTIF ecall to terminate.

Use `timeout` or `Ctrl-C`:
```bash
timeout 10 spike --isa=rv32ima -p2 -m... mempool_ipc_phase0_spike_stripped.elf
```

### Spike: Barrier deadlock (one hart spinning forever)

If you see one hart stuck at the `amoadd.w.aq` / `lw` / `bltu` loop
near `main+0x24`, the barrier count doesn't match the number of Spike
harts.

**Fix:** Ensure `SPIKE_NUM_CORES` matches `-p<N>`:
```bash
make run-spike SPIKE_NUM_CORES=4  # Must also update -p4 in Makefile
```

### Warning: "division by zero" in `mempool_get_group_id()`

**Benign.** With `NUM_CORES=2` and `NUM_GROUPS=4`, the expression
`NUM_CORES / NUM_GROUPS = 0`. This function is unused in the test and
the compiler constant-folds it away. The warning is from GCC's frontend
analysis, not from runtime execution.

### Verifying Atomic Correctness

Inspect the generated assembly to confirm RVWMO compliance:

```bash
# Check the disassembly
riscv64-unknown-elf-objdump -d mempool_ipc_phase0_spike.elf | grep -E 'fence|amo|lr\.|sc\.'
```

**Expected patterns:**
- `fence iorw,ow` before `amoswap.w` — release store for `publish_message()`
- `fence` (full) before load — acquire load for `consume_message()`
- `fence iorw,ow` + `amoadd.w.aq` — seq_cst barrier for `hw_barrier()`

### Verifying Execution via Trace Analysis

```bash
# Capture trace
timeout 5 spike --isa=rv32ima -p2 -l \
  -m0x10000:0x40000,0x40000000:0x100000,0x80000000:0x400000 \
  mempool_ipc_phase0_spike_stripped.elf 2>/tmp/trace.log

# No exceptions = success
grep -c "exception" /tmp/trace.log
# Expected: 0

# Both harts reached main()
grep "core.*0x80000104" /tmp/trace.log | head -2
# Expected: lines from both core 0 and core 1

# Producer executed amoswap (publish)
grep "core   0.*amoswap" /tmp/trace.log | head -1
# Expected: amoswap.w at ~0x800001a4

# Consumer executed fence (acquire load in consume_message)
grep "core   1.*fence" /tmp/trace.log | head -1
# Expected: fence instruction
```

## Configuration Reference

### Makefile Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SPIKE` | `/home/dragonix/mempool/install/riscv-isa-sim/bin/spike` | Path to Spike emulator |
| `SPIKE_NUM_CORES` | `2` | Number of Spike harts |

### Compile-Time Defines (Main Build)

| Define | Value | Description |
|--------|-------|-------------|
| `NUM_CORES` | 256 | Total hardware threads |
| `NUM_GROUPS` | 4 | Core groups |
| `NUM_CORES_PER_TILE` | 4 | Cores sharing a tile |
| `BANKING_FACTOR` | 4 | L1 banks per core |
| `L1_BANK_SIZE` | 1024 | Bytes per L1 bank |
| `STACK_SIZE` | 2048 | Per-core stack size (bytes) |
| `SEQ_MEM_SIZE` | 2048 | Per-core sequential memory (bytes) |
| `BOOT_ADDR` | `0x00001000` | Boot ROM address |
| `L2_BASE` | `0x80000000` | L2 scratchpad base |
| `L2_SIZE` | `0x00400000` | L2 scratchpad size (4 MB) |

### Compile-Time Defines (Spike Build Overrides)

| Define | Value | Reason |
|--------|-------|--------|
| `NUM_CORES` | 2 | Match Spike hart count |
| `LOG2_NUM_CORES` | 1 | log2(2) |
| `LOG2_NUM_CORES_PER_GROUP` | 0 | 2/4 = 0 (integer division) |
| `L1_BANK_SIZE` | 32768 | 2×4×32768 = 256 KB L1 (fits 65 KB queue) |

### Spike Memory Regions (`-m` flag)

| Region | Spike `-m` | Purpose |
|--------|-----------|---------|
| L1 TCDM | `0x10000:0x40000` | 256 KB starting at 64K |
| Control Regs | `0x40000000:0x100000` | 1 MB (writes ignored in Spike) |
| L2 SPM | `0x80000000:0x400000` | 4 MB code + data |

## Known Limitations

1. **No HTIF termination:** `_eoc` does not call HTIF `ecall(93)`, so
   Spike never exits cleanly. The program loops through boot forever.
2. **Division-by-zero warning:** `mempool_get_group_id()` warns at
   compile time with `NUM_CORES=2` (benign, function is unused).
3. **Control registers non-functional on Spike:** Writes to `0x40000000`
   (EOC), `0x40000058` (RO cache config) are silently absorbed. No
   wake-up/sleep functionality.
4. **Single test payload:** Only verifies `0xDEADBEEF` on one
   publish/consume cycle. No stress test or multi-message throughput.
