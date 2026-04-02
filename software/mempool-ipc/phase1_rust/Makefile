# =============================================================================
# MemPool-IPC Phase 1 (Rust): Spike Emulator Build & Run
# =============================================================================
# Links the Rust static library with MemPool's C boot code (crt0.S) and
# runs on Spike (2-hart rv32ima). No Zawrs -- hardware_spin_wait degrades
# to core::hint::spin_loop() fallback via target_feature dispatch.

APP = mempool_ipc_phase1
RUNTIME_DIR = ../../runtime

# Toolchain
CC      = riscv64-unknown-elf-gcc
OBJCOPY = riscv64-unknown-elf-objcopy
OBJDUMP = riscv64-unknown-elf-objdump
CARGO   = cargo

# Spike emulator
SPIKE ?= /home/dragonix/mempool/install/riscv-isa-sim/bin/spike
SPIKE_NUM_CORES ?= 2

# Rust target (no +zawrs -- fallback path)
RUST_TARGET = riscv32imac-unknown-none-elf
RUST_LIB    = target/$(RUST_TARGET)/release/libmempool_ipc_phase1.a

# CFLAGS for Spike (2-hart config, L1 at 0x10000 to match Phase 0 convention)
SPIKE_CFLAGS = -march=rv32ima_zicsr -mabi=ilp32 -O3 -mcmodel=medany \
  -I$(RUNTIME_DIR) \
  -DNUM_CORES=$(SPIKE_NUM_CORES) -DNUM_GROUPS=4 -DNUM_CORES_PER_TILE=4 \
  -DBANKING_FACTOR=4 -DSTACK_SIZE=2048 -DXQUEUE_SIZE=64 -DSEQ_MEM_SIZE=2048 \
  -DLOG2_NUM_CORES=1 -DLOG2_NUM_GROUPS=2 -DLOG2_NUM_CORES_PER_TILE=2 \
  -DLOG2_BANKING_FACTOR=2 -DLOG2_STACK_SIZE=11 -Dstacklimit=mscratch \
  -DLOG2_NUM_CORES_PER_GROUP=0 -DLOG2_NUM_BANKS_PER_TILE=4 -DLOG2_SEQ_MEM_SIZE=11 \
  -DL1_BASE=0x00010000 -DL1_SIZE=0x00040000 \
  -DL2_BASE=0x80000000 -DL2_SIZE=0x00400000 \
  -DBOOT_ADDR=0x00001000 \
  -DL1_BANK_SIZE=32768

# Spike physical memory map (must cover all linker regions):
#   L1:   0x10000  256KB  (TCDM -- stacks, ipc_queue, boot_barrier)
#   CTRL: 0x40000000 1MB  (MemPool control regs -- crt0 writes wake-up addr here)
#   L2:   0x80000000 4MB  (.text, .rodata, .data, .htif)
# NOTE: Do NOT map 0x1000 -- Spike uses its own boot ROM at that address.
SPIKE_MEM = -m0x10000:0x40000,0x40000000:0x100000,0x80000000:0x400000

# --- Build Steps ---

.PHONY: all run-spike clean

all: $(APP)_spike_stripped.elf

RUST_SRCS = src/lib.rs src/main.rs src/primitives.rs src/hw.rs src/sync.rs src/alloc_ffi.rs build.rs

# 1. Build the Rust static library (no Zawrs -- fallback path)
$(RUST_LIB): $(RUST_SRCS)
	NUM_CORES=$(SPIKE_NUM_CORES) $(CARGO) build --release --target $(RUST_TARGET)

# 2. Assemble boot code (crt0.S)
spike_crt0.o: $(RUNTIME_DIR)/crt0.S
	$(CC) $(SPIKE_CFLAGS) -c $< -o $@

# 3. Assemble HTIF symbols for Spike
spike_htif.o: spike_htif.S
	$(CC) $(SPIKE_CFLAGS) -c $< -o $@

# 4. Assemble L1 TCDM globals (ipc_queue, boot_barrier)
spike_globals.o: spike_globals.S
	$(CC) $(SPIKE_CFLAGS) -c $< -o $@

# 5. Generate arch.ld with Spike L1 addresses
#    arch.ld.c hardcodes L1 ORIGIN=0x00000000; sed rewrites to 0x10000
spike_arch.ld: $(RUNTIME_DIR)/arch.ld.c
	$(CC) $(SPIKE_CFLAGS) -E -P -x c $< -o $@
	sed -i 's/ORIGIN = 0x00000000/ORIGIN = 0x00010000/' $@

# 6. Link: crt0.o + globals.o + htif.o + Rust .a -> ELF
$(APP)_spike.elf: spike_crt0.o spike_globals.o spike_htif.o spike_arch.ld $(RUST_LIB)
	$(CC) $(SPIKE_CFLAGS) -L. -T spike_link.ld -nostdlib -nostartfiles \
	  -Wl,--no-warn-rwx-segments -o $@ \
	  spike_crt0.o spike_globals.o spike_htif.o $(RUST_LIB) -lgcc

# 7. Strip L1 NOLOAD sections that Spike's ELF loader cannot handle
$(APP)_spike_stripped.elf: $(APP)_spike.elf
	$(OBJCOPY) --remove-section=.l1_seq --remove-section=.l1 $< $@

# 8. Disassembly
$(APP).dump: $(APP)_spike.elf
	$(OBJDUMP) -d $< > $@

# 9. Run on Spike (2 harts, rv32ima, matching Phase 0 memory map)
run-spike: $(APP)_spike.elf
	@echo "Running Phase 1 Rust on Spike ($(SPIKE_NUM_CORES) harts, rv32imac)..."
	-timeout --foreground 120 $(SPIKE) --isa=rv32imac -p$(SPIKE_NUM_CORES) \
	  $(SPIKE_MEM) \
	  $(APP)_spike.elf

clean:
	rm -f spike_crt0.o spike_htif.o spike_globals.o spike_arch.ld
	rm -f $(APP)_spike.elf $(APP)_spike_stripped.elf $(APP).elf $(APP).dump
	rm -f link.map
	$(CARGO) clean
