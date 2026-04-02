# dataplane-emu — Phase 5 Hardware Offloading Data Plane Emulator
#
# Build targets:
#   make            — Build for host (x86_64, functional verification)
#   make run        — Build and run on host
#   make clean      — Remove build artifacts

CXX      ?= g++
CXXFLAGS := -std=c++17 -O2 -Wall -Wextra -Wpedantic
INCLUDES := -Iinclude
LDFLAGS  := -pthread

APP := dataplane_emu

SRCS := src/main.cpp
OBJS := $(SRCS:.cpp=.o)

.PHONY: all run clean

all: $(APP)

$(APP): $(OBJS)
	$(CXX) $(CXXFLAGS) $(LDFLAGS) -o $@ $^

src/%.o: src/%.cpp
	$(CXX) $(CXXFLAGS) $(INCLUDES) -c -o $@ $<

run: $(APP)
	@echo "Running Phase 5 data plane emulator (host mode)..."
	./$(APP)

clean:
	rm -f $(APP) $(OBJS)
