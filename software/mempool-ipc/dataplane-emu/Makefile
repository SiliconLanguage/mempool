# dataplane-emu -- Phase 5 Hardware Offloading Data Plane Emulator
#
# Build targets:
#   make                  -- Build for host (x86_64, functional verification)
#   make run              -- Build and run on host
#   make build-arm64-aws  -- Cross-compile for AWS Graviton3 (Neoverse-V1)
#   make build-arm64-azure -- Cross-compile for Azure Cobalt 100 (Neoverse-N2)
#   make run-arm64-native -- Run natively on ARM64 host with hugepages
#   make run-qemu-arm64   -- Emulate ARM64 via QEMU user-mode
#   make clean            -- Remove build artifacts

# --- Host toolchain ---
CXX      ?= g++
CXXFLAGS := -std=c++17 -O2 -Wall -Wextra -Wpedantic
INCLUDES := -Iinclude
LDFLAGS  := -pthread

# --- ARM64 cross-compilation ---
ARM64_CXX     ?= aarch64-linux-gnu-g++
ARM64_CXXFLAGS := -std=c++17 -O2 -Wall -Wextra -Wpedantic -march=armv8.4-a
ARM64_LDFLAGS  := -pthread -static

APP       := dataplane_emu
APP_ARM64 := $(APP)_arm64

SRCS := src/main.cpp
OBJS := $(SRCS:.cpp=.o)
OBJS_ARM64 := $(SRCS:src/%.cpp=src/%_arm64.o)

.PHONY: all run clean build-arm64-aws build-arm64-azure run-arm64-native run-qemu-arm64

all: $(APP)

# --- Host build ---
$(APP): $(OBJS)
	$(CXX) $(CXXFLAGS) $(LDFLAGS) -o $@ $^

src/%.o: src/%.cpp
	$(CXX) $(CXXFLAGS) $(INCLUDES) -c -o $@ $<

run: $(APP)
	@echo "Running Phase 5 data plane emulator (host mode)..."
	./$(APP)

# --- ARM64 cross-compile targets ---

# AWS Graviton3 (Neoverse-V1): ARMv8.4-a + SVE + BF16
build-arm64-aws: CXXFLAGS_PLAT := -mcpu=neoverse-v1
build-arm64-aws: $(SRCS)
	@echo "Cross-compiling for AWS Graviton3 (Neoverse-V1)..."
	$(ARM64_CXX) $(ARM64_CXXFLAGS) $(CXXFLAGS_PLAT) $(INCLUDES) $(ARM64_LDFLAGS) -o $(APP_ARM64) $^

# Azure Cobalt 100 (Neoverse-N2): ARMv9.0-a + SVE2
build-arm64-azure: CXXFLAGS_PLAT := -mcpu=neoverse-n2
build-arm64-azure: $(SRCS)
	@echo "Cross-compiling for Azure Cobalt 100 (Neoverse-N2)..."
	$(ARM64_CXX) $(ARM64_CXXFLAGS) $(CXXFLAGS_PLAT) $(INCLUDES) $(ARM64_LDFLAGS) -o $(APP_ARM64) $^

# --- ARM64 run targets ---

# Run natively on ARM64 host (requires hugepages for zero-copy DMA)
run-arm64-native: $(APP_ARM64)
	@echo "Running Phase 5 C++ Data Plane Natively on ARM64..."
	@echo "Ensuring Hugepages are allocated..."
	sudo HUGEMEM=2048 ./scripts/setup.sh
	-timeout --foreground 120 sudo ./$(APP_ARM64)

# Emulate ARM64 via QEMU user-mode (no hardware required)
run-qemu-arm64: $(APP_ARM64)
	@echo "Emulating Phase 5 ARM64 Data Plane via QEMU..."
	-timeout --foreground 120 qemu-aarch64 -cpu max ./$(APP_ARM64)

# --- Cleanup ---
clean:
	rm -f $(APP) $(APP_ARM64) $(OBJS) $(OBJS_ARM64)
