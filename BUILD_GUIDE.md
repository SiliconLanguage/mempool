# MemPool Build & Run Guide — From Source to Simulation

This document records every step, issue, and resolution encountered while building the MemPool toolchain and running `hello_world` on the Verilated RTL model on a fresh Ubuntu 24.04 system **without sudo access** (except where noted).

---

## Environment

| Component       | Version                                   |
|-----------------|-------------------------------------------|
| Host OS         | Ubuntu 24.04 (WSL2, kernel 6.6.87)       |
| Host GCC        | 13.3.0                                    |
| Python          | 3.12.3                                    |
| CMake           | 3.28.3                                    |
| Configuration   | `minpool` (16 cores, 4 tiles, 4 cores/tile) |

All locally-built dependencies are installed under `~/.local` (bin, lib, include).

---

## Table of Contents

1. [Phase 1: Host Dependencies](#phase-1-host-dependencies)
2. [Phase 2: RISC-V GCC Toolchain](#phase-2-risc-v-gcc-toolchain)
3. [Phase 3: Cross-Compile hello_world](#phase-3-cross-compile-hello_world)
4. [Phase 4: Spike ISA Simulator (Failed Path)](#phase-4-spike-isa-simulator-failed-path)
5. [Phase 5: Verilator RTL Simulation (Successful Path)](#phase-5-verilator-rtl-simulation-successful-path)
6. [Phase 6: Running the Simulation](#phase-6-running-the-simulation)
7. [Quick Reference Commands](#quick-reference-commands)

---

## Phase 1: Host Dependencies

The system lacked several build tools required by the RISC-V GCC toolchain and Verilator. These were built from source into `~/.local/` since we had no sudo access.

### 1.1 GNU M4

**Why:** Required by Bison and autoconf.

```bash
cd /tmp
curl -O https://ftp.gnu.org/gnu/m4/m4-1.4.19.tar.xz
tar xf m4-1.4.19.tar.xz && cd m4-1.4.19
./configure --prefix=$HOME/.local && make -j4 && make install
```

### 1.2 GNU Bison

**Why:** The RISC-V GCC build requires Bison for parser generation.

```bash
cd /tmp
curl -O https://ftp.gnu.org/gnu/bison/bison-3.8.2.tar.xz
tar xf bison-3.8.2.tar.xz && cd bison-3.8.2
./configure --prefix=$HOME/.local && make -j4 && make install
```

### 1.3 Flex

**Why:** Required by GCC and Verilator builds.

```bash
cd /tmp
curl -LO https://github.com/westes/flex/releases/download/v2.6.4/flex-2.6.4.tar.gz
tar xf flex-2.6.4.tar.gz && cd flex-2.6.4
./configure --prefix=$HOME/.local && make -j4 && make install
```

### 1.4 Texinfo

**Why:** GCC's `makeinfo` command comes from texinfo.

```bash
cd /tmp
curl -O https://ftp.gnu.org/gnu/texinfo/texinfo-7.1.tar.xz
tar xf texinfo-7.1.tar.xz && cd texinfo-7.1
./configure --prefix=$HOME/.local && make -j4 && make install
```

### 1.5 GMP, MPFR, MPC

**Why:** GCC's multi-precision arithmetic dependencies.

```bash
# GMP
cd /tmp
curl -O https://ftp.gnu.org/gnu/gmp/gmp-6.3.0.tar.xz
tar xf gmp-6.3.0.tar.xz && cd gmp-6.3.0
./configure --prefix=$HOME/.local && make -j4 && make install

# MPFR
cd /tmp
curl -O https://ftp.gnu.org/gnu/mpfr/mpfr-4.2.1.tar.xz
tar xf mpfr-4.2.1.tar.xz && cd mpfr-4.2.1
./configure --prefix=$HOME/.local --with-gmp=$HOME/.local && make -j4 && make install

# MPC
cd /tmp
curl -O https://ftp.gnu.org/gnu/mpc/mpc-1.3.1.tar.gz
tar xf mpc-1.3.1.tar.gz && cd mpc-1.3.1
./configure --prefix=$HOME/.local --with-gmp=$HOME/.local --with-mpfr=$HOME/.local && make -j4 && make install
```

### 1.6 GNU Autoconf 2.71

**Why:** Verilator's `autoconf` step requires a modern autoconf (system had none).

```bash
cd /tmp
curl -O https://ftp.gnu.org/gnu/autoconf/autoconf-2.71.tar.xz
tar xf autoconf-2.71.tar.xz && cd autoconf-2.71
./configure --prefix=$HOME/.local && make -j4 && make install
```

### 1.7 Device Tree Compiler (dtc)

**Why:** Spike's `configure` checks for `dtc` and refuses to build without it.

```bash
cd /tmp
git clone https://git.kernel.org/pub/scm/utils/dtc/dtc.git && cd dtc
make PREFIX=$HOME/.local install
```

> **Note:** The above `dtc` build and install into `~/.local` was verified to work on Ubuntu 24.04 without sudo. If you _do_ have sudo and prefer to use the distro package instead of building from source, you can alternatively run `sudo apt-get install device-tree-compiler` and skip the manual `dtc` build.

### 1.8 libelf Development Headers

**Why:** The Verilated C++ model includes `<libelf.h>` (in `dpi_memutil.cc`), but only the runtime `libelf.so.1` was installed — no headers.

**Issue:** `dpi_memutil.cc:11: fatal error: libelf.h: No such file or directory`

**Resolution:** Extract headers from the `.deb` package without needing sudo:

```bash
cd /tmp
apt download libelf-dev
mkdir -p elfdev && dpkg-deb -x libelf-dev_*.deb elfdev/
cp elfdev/usr/include/libelf.h elfdev/usr/include/gelf.h elfdev/usr/include/nlist.h ~/.local/include/
cp -r elfdev/usr/include/elfutils ~/.local/include/
```

Also create the linker symlink:

```bash
ln -sf /usr/lib/x86_64-linux-gnu/libelf-0.190.so ~/.local/lib/libelf.so
```

---

## Phase 2: RISC-V GCC Toolchain

### 2.1 Initialize Submodule

```bash
cd mempool
git submodule update --init --recursive toolchain/riscv-gnu-toolchain
```

**Issue:** The initial `git submodule update --init` failed due to a corrupted checkout. Fixed by:

```bash
rm -rf toolchain/riscv-gnu-toolchain
git submodule update --init toolchain/riscv-gnu-toolchain
cd toolchain/riscv-gnu-toolchain
git submodule update --init --recursive
```

### 2.2 Build GCC

```bash
export PATH="$HOME/.local/bin:$PATH"
export CFLAGS="-I$HOME/.local/include"
export CXXFLAGS="-I$HOME/.local/include"
export LDFLAGS="-L$HOME/.local/lib"
export LD_LIBRARY_PATH="$HOME/.local/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

make tc-riscv-gcc
```

This runs `configure --prefix=install/riscv-gcc --with-arch=rv32im --with-cmodel=medlow --enable-multilib`, followed by a full GCC cross-compiler build. Takes ~30–60 minutes.

**Issues encountered:**
- **Missing bison/flex/texinfo:** Resolved in Phase 1 (built from source).
- **Missing GMP/MPFR/MPC headers:** Resolved by setting `CFLAGS`/`LDFLAGS` to `~/.local`.
- **`-fpermissive` needed:** Some GCC bootstrap sources triggered errors with GCC 13's stricter defaults. Fixed by adding `-fpermissive` to `CFLAGS`/`CXXFLAGS`.

**Result:** `install/riscv-gcc/bin/riscv32-unknown-elf-gcc` (GCC 7.1.1) installed successfully.

---

## Phase 3: Cross-Compile hello_world

```bash
export PATH="$PWD/install/riscv-gcc/bin:$PATH"
config=minpool make -C software/apps/baremetal hello_world
```

**Result:** `software/bin/apps/baremetal/hello_world` — a 32-bit RISC-V ELF compiled for 16 cores.

---

## Phase 4: Spike ISA Simulator (Failed Path)

> **This path was abandoned.** It is documented here for reference only.

### 4.1 Build Spike

```bash
make riscv-isa-sim
```

**Issue:** `configure: error: device-tree-compiler not found` → Resolved by installing `dtc` (Phase 1.7).

### 4.2 Attempted Execution

```bash
spike --isa=RV32IMA /path/to/hello_world
```

**Issue 1 — `trap_store_access_fault`:** The MemPool ELF has a TCDM segment at address `0x0`, which overlaps Spike's boot ROM at `0x1000`.

**Issue 2 — `trap_illegal_instruction` at `csrw 0x7d1, t0`:** After stripping the TCDM segment, Spike trapped on the Snitch-specific `stacklimit` CSR (0x7D1), which Spike does not implement.

**Conclusion:** The MemPool binary requires the full RTL hardware (custom CSRs, TCDM memory map, multi-core wake-up logic). Spike cannot simulate it.

---

## Phase 5: Verilator RTL Simulation (Successful Path)

### 5.1 Install Bender

```bash
make bender
```

Installs Bender v0.28.2 to `install/bender/bender`.

### 5.2 Checkout Hardware Dependencies

```bash
make update-deps
```

Runs `bender checkout`, cloning ~10 PULP IP repositories (common_cells, axi, fpnew, cluster_interconnect, etc.) into `hardware/deps/`.

### 5.3 Initialize the Verilator Submodule

```bash
git submodule update --init toolchain/verilator
```

The repository pinned version is **v4.218**, but it had a critical bug (see 5.5).

### 5.4 Build Verilator

```bash
cd toolchain/verilator
autoconf
CC=gcc CXX=g++ \
  CXXFLAGS="-include memory -I$HOME/.local/include" \
  ./configure --prefix=$PWD/../../install/verilator
make -j4
make install
```

**Issue 1 — Missing `FlexLexer.h`:** The Verilator build needs `FlexLexer.h` from Flex. It was installed at `~/.local/include/FlexLexer.h` but not in the default search path.
**Fix:** Add `-I$HOME/.local/include` to `CXXFLAGS`.

**Issue 2 — `std::unique_ptr` not found:** GCC 13 removed the transitive `#include <memory>` from other standard headers. Verilator v4.218/v4.228 source files assumed it was transitively included.
**Fix:** Add `-include memory` to `CXXFLAGS`.

### 5.5 Verilator v4.218 Internal Fault → Upgrade to v4.228

When running the RTL-to-C++ translation with v4.218:

```
%Error: Internal Error: ... V3AssertPre.cpp ...
```

The `assertPreAll` Verilator pass crashed on the fpnew FPU RTL (LITENDIAN vectors). This is a known Verilator v4.218 bug.

**Resolution:** Upgrade to v4.228:

```bash
cd toolchain/verilator
git checkout v4.228
# Rebuild with same flags as 5.4
autoconf
CXXFLAGS="-include memory -I$HOME/.local/include" \
  ./configure --prefix=$PWD/../../install/verilator
make -j4 && make install
```

### 5.6 Disable `--hierarchical` Mode

The `hardware/tb/verilator/verilator.flags` file contained `--hierarchical`, which contributed to crashes with the MinPool configuration.

**Fix:** Comment out line 44 in `hardware/tb/verilator/verilator.flags`:

```diff
- --hierarchical
+ // --hierarchical  # Disabled for MinPool configuration (hierarchical mode causes crashes)
```

### 5.7 Build the Verilated Model

```bash
cd hardware
export PATH="$HOME/.local/bin:$PWD/../install/riscv-gcc/bin:$PWD/../install/verilator/bin:$PWD/../install/bender:$PATH"
export CPATH="$HOME/.local/include"
export LIBRARY_PATH="$HOME/.local/lib"
export LD_LIBRARY_PATH="$HOME/.local/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

config=minpool make verilate
```

This does two things:
1. **RTL → C++ translation:** Bender generates a Verilator file list, then Verilator converts all SystemVerilog RTL into C++ sources in `hardware/verilator_build/`.
2. **C++ compilation:** Compiles the generated C++ into the `Vmempool_tb_verilator` executable (~827 MB with debug info).

**Issue — Missing `libelf.h`:** The `dpi_memutil.cc` file includes `<libelf.h>` for ELF binary loading. Resolved in Phase 1.8.

**Key environment variables:**
- `CPATH=$HOME/.local/include` — makes `libelf.h` visible to the C++ compiler.
- `LIBRARY_PATH=$HOME/.local/lib` — makes `-lelf` linkable.
- `LD_LIBRARY_PATH=$HOME/.local/lib` — makes `libelf.so` loadable at runtime.

---

## Phase 6: Running the Simulation

### 6.1 Run hello_world

```bash
cd hardware
export PATH="$HOME/.local/bin:$PWD/../install/riscv-gcc/bin:$PWD/../install/verilator/bin:$PWD/../install/bender:$PATH"
export LD_LIBRARY_PATH="$HOME/.local/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

config=minpool app=baremetal/hello_world make verilate
```

### 6.2 Expected Output

```
Simulation of MemPool
=====================

Simulation running, end by pressing CTRL-c.
[Tracer] Logging Hart          0 to trace_hart_0x00000000.dasm
[Tracer] Logging Hart          1 to trace_hart_0x00000001.dasm
...
[Tracer] Logging Hart         15 to trace_hart_0x0000000f.dasm
[UART] Core   0 says Hello!
[UART] Core   1 says Hello!
[UART] Core   2 says Hello!
...
[UART] Core  15 says Hello!
[EOC] Simulation ended at      54272 (retval = 0).
- .../mempool_tb_verilator.sv:104: Verilog $finish
Received $finish() from Verilog, shutting down simulation.

Simulation statistics
=====================
Executed cycles:  27136
Wallclock time:   18.876 s
Simulation speed: 1437.59 cycles/s (1.43759 kHz)
```

All 16 Snitch cores boot, print via the fake UART, and exit cleanly with return value 0.

### 6.3 Output Artifacts

| File | Location | Description |
|------|----------|-------------|
| Transcript | `hardware/build/transcript` | Full simulation console log |
| Traces | `hardware/build/trace_hart_0x*.dasm` | Per-core instruction traces |
| Executable | `hardware/verilator_build/Vmempool_tb_verilator` | Verilated model binary |

---

## Quick Reference Commands

```bash
# === Environment Setup (run once per shell) ===
export PATH="$HOME/.local/bin:$PWD/install/riscv-gcc/bin:$PWD/install/verilator/bin:$PWD/install/bender:$PATH"
export LD_LIBRARY_PATH="$HOME/.local/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export CPATH="$HOME/.local/include"
export LIBRARY_PATH="$HOME/.local/lib"

# === Build Toolchain (one-time) ===
make bender                         # Install Bender
make update-deps                    # Checkout RTL dependencies
make tc-riscv-gcc                   # Build RISC-V GCC cross-compiler

# === Build Verilator (one-time) ===
cd toolchain/verilator && git checkout v4.228
autoconf && CXXFLAGS="-include memory -I$HOME/.local/include" \
  ./configure --prefix=$PWD/../../install/verilator && make -j4 && make install
cd ../..

# === Compile Software ===
config=minpool make -C software/apps/baremetal hello_world

# === Run Simulation ===
cd hardware
config=minpool app=baremetal/hello_world make verilate
```

---

## Summary of All Issues and Resolutions

| # | Issue | Root Cause | Resolution |
|---|-------|------------|------------|
| 1 | Missing bison, flex, texinfo, m4 | No dev packages, no sudo | Built from source → `~/.local` |
| 2 | Missing GMP/MPFR/MPC | No dev packages | Built from source → `~/.local` |
| 3 | GCC bootstrap `-fpermissive` errors | GCC 13 stricter than GCC 7 sources expect | Added `-fpermissive` to CFLAGS/CXXFLAGS |
| 4 | `riscv-gnu-toolchain` submodule corrupt | Partial clone state | `rm -rf` + fresh `git submodule update --init` |
| 5 | Spike: `trap_store_access_fault` | TCDM at 0x0 overlaps Spike boot ROM | Abandoned Spike path |
| 6 | Spike: `trap_illegal_instruction` (CSR 0x7D1) | Snitch custom `stacklimit` CSR | Abandoned Spike path |
| 7 | Missing `dtc` for Spike build | No `device-tree-compiler` package | Built dtc from source (or `sudo apt install`) |
| 8 | Missing `FlexLexer.h` | Flex headers in `~/.local` not in search path | `-I$HOME/.local/include` in CXXFLAGS |
| 9 | `std::unique_ptr` not found | GCC 13 removed transitive `<memory>` include | `-include memory` in CXXFLAGS |
| 10 | Verilator v4.218 internal fault | Bug in V3AssertPre on fpnew LITENDIAN vectors | Upgraded to v4.228 |
| 11 | `--hierarchical` Verilator crash | MinPool config incompatible with hierarchical mode | Commented out in `verilator.flags` |
| 12 | Missing `libelf.h` | No `libelf-dev`, no sudo | Extracted from `.deb` via `dpkg-deb -x` |
| 13 | Missing `libelf.so` linker symlink | Only `libelf.so.1` runtime present | `ln -sf` to `~/.local/lib/libelf.so` |
| 14 | Autoconf missing for Verilator | No autoconf on system | Built GNU autoconf 2.71 from source |
