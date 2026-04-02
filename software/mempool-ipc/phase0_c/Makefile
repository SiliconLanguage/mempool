# =============================================================================
# MemPool-IPC Phase 0: C-Subset Ground Truth
# =============================================================================

APP = mempool_ipc_phase0

# Our Data Plane Sources
SRCS = src/main.c

# MemPool Bare-Metal Runtime Sources
RUNTIME_DIR = ../../runtime

# We only need the assembly boot code. No bloated string or printf libraries!
RUNTIME_SRCS = $(RUNTIME_DIR)/crt0.S

# Explicitly map the objects to prevent 'make clean' from deleting source files
RUNTIME_OBJS = $(RUNTIME_DIR)/crt0.o

OBJS = src/main.o $(RUNTIME_OBJS)

# Toolchain
CC = riscv64-unknown-elf-gcc
OBJDUMP = riscv64-unknown-elf-objdump

# Compiler Flags:
# CFLAGS = -march=rv32ima_zicsr -mabi=ilp32 -O3 -mcmodel=medany -Wall -Wextra -Iinclude -I$(RUNTIME_DIR) \
#          -DNUM_CORES=256 -DNUM_GROUPS=4 -DNUM_CORES_PER_TILE=4 \
#          -DBANKING_FACTOR=4 -DSTACK_SIZE=2048 -DXQUEUE_SIZE=64 -DSEQ_MEM_SIZE=2048 \
#          -DLOG2_NUM_CORES=8 -DLOG2_NUM_GROUPS=2 -DLOG2_NUM_CORES_PER_TILE=2 \
#          -DLOG2_BANKING_FACTOR=2 -DLOG2_STACK_SIZE=11 -Dstacklimit=mscratch \
#          -DLOG2_NUM_CORES_PER_GROUP=6 -DLOG2_NUM_BANKS_PER_TILE=4 -DLOG2_SEQ_MEM_SIZE=11

# Compiler Flags:
CFLAGS = -march=rv32ima_zicsr -mabi=ilp32 -O3 -mcmodel=medany -Wall -Wextra -Iinclude -I$(RUNTIME_DIR) \
         -DNUM_CORES=256 -DNUM_GROUPS=4 -DNUM_CORES_PER_TILE=4 \
         -DBANKING_FACTOR=4 -DSTACK_SIZE=2048 -DXQUEUE_SIZE=64 -DSEQ_MEM_SIZE=2048 \
         -DLOG2_NUM_CORES=8 -DLOG2_NUM_GROUPS=2 -DLOG2_NUM_CORES_PER_TILE=2 \
         -DLOG2_BANKING_FACTOR=2 -DLOG2_STACK_SIZE=11 -Dstacklimit=mscratch \
         -DLOG2_NUM_CORES_PER_GROUP=6 -DLOG2_NUM_BANKS_PER_TILE=4 -DLOG2_SEQ_MEM_SIZE=11 \
         -DL1_BASE=0x10000000 -DL1_SIZE=0x00100000 \
         -DL2_BASE=0x80000000 -DL2_SIZE=0x00400000 \
         -DROM_BASE=0x00001000 -DROM_SIZE=0x00001000 \
         -DBOOT_ADDR=0x00001000 \
         -DL1_BANK_SIZE=1024

# Linker Flags:
# -L. tells the linker to look in the current directory for our dynamically generated arch.ld
LDFLAGS = -L. -T $(RUNTIME_DIR)/link.ld -nostdlib -nostartfiles -lgcc \
          -Wl,--no-warn-rwx-segments

all: arch.ld $(APP).elf $(APP).dump

# 1. Generate the hardware-specific linker script via the C Preprocessor
arch.ld: $(RUNTIME_DIR)/arch.ld.c
	$(CC) $(CFLAGS) -E -P -x c $< -o $@

# 2. Link the final executable (Depends on arch.ld, but only passes OBJS to the linker)
$(APP).elf: $(OBJS) arch.ld
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $(OBJS)

# 3. Generate disassembly for RVWMO atomic verification
$(APP).dump: $(APP).elf
	$(OBJDUMP) -d $< > $@

# Compile C files
%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

# Compile Assembly files (boot code)
%.o: %.S
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -f $(OBJS) $(APP).elf $(APP).dump arch.ld

# =============================================================================
# Emulator Hooks
# =============================================================================
run-banshee: arch.ld $(APP).elf
	banshee --configuration ../../config/mempool_256.json $(APP).elf

# =============================================================================
# Spike Emulator Hooks (Option C: functional validation with 2 harts)
# =============================================================================
SPIKE ?= /home/dragonix/mempool/install/riscv-isa-sim/bin/spike
OBJCOPY = riscv64-unknown-elf-objcopy
SPIKE_NUM_CORES ?= 2

# Spike-specific CFLAGS: override NUM_CORES and dependent macros for 2-hart testing
SPIKE_CFLAGS = $(subst -DNUM_CORES=256,-DNUM_CORES=$(SPIKE_NUM_CORES), \
               $(subst -DLOG2_NUM_CORES=8,-DLOG2_NUM_CORES=1, \
               $(subst -DLOG2_NUM_CORES_PER_GROUP=6,-DLOG2_NUM_CORES_PER_GROUP=0, \
               $(subst -DL1_BANK_SIZE=1024,-DL1_BANK_SIZE=32768, \
               $(CFLAGS)))))

spike_htif.o: spike_htif.S
	$(CC) $(CFLAGS) -c $< -o $@

spike_arch.ld: $(RUNTIME_DIR)/arch.ld.c
	$(CC) $(SPIKE_CFLAGS) -E -P -x c $< -o $@
	sed -i 's/ORIGIN = 0x00000000/ORIGIN = 0x00010000/' $@

spike_src/main.o: src/main.c
	@mkdir -p spike_src
	$(CC) $(SPIKE_CFLAGS) -c $< -o $@

spike_crt0.o: $(RUNTIME_DIR)/crt0.S
	$(CC) $(SPIKE_CFLAGS) -c $< -o $@

$(APP)_spike.elf: spike_src/main.o spike_crt0.o spike_htif.o spike_arch.ld
	$(CC) $(SPIKE_CFLAGS) -L. -T spike_link.ld -nostdlib -nostartfiles -lgcc \
	  -Wl,--no-warn-rwx-segments -o $@ spike_src/main.o spike_crt0.o spike_htif.o

# Strip L1 NOLOAD sections that Spike's ELF loader cannot handle
$(APP)_spike_stripped.elf: $(APP)_spike.elf
	$(OBJCOPY) --remove-section=.l1_seq --remove-section=.l1 $< $@

run-spike: $(APP)_spike_stripped.elf
	@echo "Spike will loop indefinitely (no HTIF _eoc). Use Ctrl-C or wait for timeout."
	-timeout 10 $(SPIKE) --isa=rv32ima -p$(SPIKE_NUM_CORES) \
	  -m0x10000:0x40000,0x40000000:0x100000,0x80000000:0x400000 \
	  $<

clean-spike:
	rm -f spike_htif.o spike_crt0.o spike_arch.ld $(APP)_spike.elf $(APP)_spike_stripped.elf
	rm -rf spike_src
