#!/usr/bin/env bash
# install-prereqs.sh — Install prerequisites for MemPool-IPC Phase 1 (Rust)
#
# Usage: ./install-prereqs.sh
#
# Installs:
#   1. Rust toolchain + riscv32imac-unknown-none-elf target
#   2. RISC-V GCC cross-compiler (riscv64-unknown-elf-gcc)
#   3. QEMU with riscv32 system emulation (Zawrs support requires QEMU >= 8.0)
#
# The MemPool-specific tools (Spike, Verilator) are built from the repo's
# toolchain/ directory — see the top-level BUILD_GUIDE.md for those.

set -euo pipefail

info()  { printf '\033[1;34m[INFO]\033[0m  %s\n' "$1"; }
warn()  { printf '\033[1;33m[WARN]\033[0m  %s\n' "$1"; }
error() { printf '\033[1;31m[ERROR]\033[0m %s\n' "$1"; exit 1; }

# --------------------------------------------------------------------------
# 1. Rust toolchain
# --------------------------------------------------------------------------
if command -v rustup &>/dev/null; then
    info "rustup found: $(rustup --version 2>/dev/null | head -1)"
else
    info "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
fi

info "Ensuring stable toolchain is installed..."
rustup toolchain install stable

info "Adding riscv32imac-unknown-none-elf target..."
rustup target add riscv32imac-unknown-none-elf

info "Rust: $(rustc --version)"

# --------------------------------------------------------------------------
# 2. RISC-V GCC cross-compiler
# --------------------------------------------------------------------------
if command -v riscv64-unknown-elf-gcc &>/dev/null; then
    info "riscv64-unknown-elf-gcc found: $(riscv64-unknown-elf-gcc --version | head -1)"
else
    info "Installing RISC-V GCC cross-compiler..."
    if command -v apt-get &>/dev/null; then
        sudo apt-get update
        sudo apt-get install -y gcc-riscv64-unknown-elf binutils-riscv64-unknown-elf
    elif command -v brew &>/dev/null; then
        brew install riscv-gnu-toolchain
    else
        warn "No supported package manager found. Install riscv64-unknown-elf-gcc manually."
        warn "See: https://github.com/riscv-collab/riscv-gnu-toolchain"
    fi
fi

# --------------------------------------------------------------------------
# 3. QEMU (riscv32 system emulation, Zawrs requires >= 8.0)
# --------------------------------------------------------------------------
if command -v qemu-system-riscv32 &>/dev/null; then
    info "qemu-system-riscv32 found: $(qemu-system-riscv32 --version | head -1)"
else
    info "Installing QEMU system emulation for RISC-V..."
    if command -v apt-get &>/dev/null; then
        sudo apt-get update
        sudo apt-get install -y qemu-system-misc
    elif command -v brew &>/dev/null; then
        brew install qemu
    else
        warn "No supported package manager found. Install qemu-system-riscv32 (>= 8.0) manually."
        warn "See: https://www.qemu.org/download/"
    fi
fi

# Verify QEMU version >= 8.0 for Zawrs support
if command -v qemu-system-riscv32 &>/dev/null; then
    QEMU_VER=$(qemu-system-riscv32 --version | grep -oP '\d+\.\d+' | head -1)
    QEMU_MAJOR=$(echo "$QEMU_VER" | cut -d. -f1)
    if [[ "$QEMU_MAJOR" -lt 8 ]]; then
        warn "QEMU $QEMU_VER detected — Zawrs support requires >= 8.0"
        warn "The 'make run-qemu' target may not work. Consider upgrading QEMU."
    else
        info "QEMU $QEMU_VER — Zawrs support confirmed."
    fi
fi

# --------------------------------------------------------------------------
# 4. GNU Make (should already be present)
# --------------------------------------------------------------------------
if ! command -v make &>/dev/null; then
    info "Installing GNU Make..."
    if command -v apt-get &>/dev/null; then
        sudo apt-get install -y make
    fi
fi

# --------------------------------------------------------------------------
# Summary
# --------------------------------------------------------------------------
echo ""
info "=== Prerequisites Summary ==="
info "Rust:       $(rustc --version 2>/dev/null || echo 'NOT FOUND')"
info "RV target:  $(rustup target list --installed 2>/dev/null | grep riscv || echo 'NOT FOUND')"
info "RV GCC:     $(riscv64-unknown-elf-gcc --version 2>/dev/null | head -1 || echo 'NOT FOUND')"
info "QEMU:       $(qemu-system-riscv32 --version 2>/dev/null | head -1 || echo 'NOT FOUND')"
info "Make:       $(make --version 2>/dev/null | head -1 || echo 'NOT FOUND')"
echo ""
info "Build with:  make all       (Spike, no Zawrs)"
info "             make qemu      (QEMU, with Zawrs)"
info "Run with:    make run-spike"
info "             make run-qemu"
