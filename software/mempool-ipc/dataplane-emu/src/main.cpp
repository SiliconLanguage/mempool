// Phase 5 — Hardware Offloading Data Plane Emulator
//
// Demonstrates the unified SPSC queue with hardware-assisted intrinsics
// (sl_wait_on_address + sl_load_nt) across RISC-V and ARM64.
//
// On x86_64 hosts, the intrinsics are compile-time no-ops; the queue
// falls back to std::atomic acquire/release for functional verification.

#include <cstdio>
#include <cstdint>
#include <thread>
#include <atomic>
#include "dataplane_emu/spsc_queue.h"

static constexpr uint64_t TEST_PAYLOAD = 0xDEAD'BEEF'CAFE'BABE;
static constexpr size_t   NUM_MESSAGES = 16;

int main() {
    dataplane_emu::SpscQueue<1024> queue;
    std::atomic<bool> consumer_ok{true};

    // Consumer thread
    std::thread consumer([&]() {
        for (size_t i = 0; i < NUM_MESSAGES; ++i) {
            uint64_t msg;
            // Use try_consume (portable fallback) on x86_64 hosts
            // where sl_wait_on_address is a no-op and consume() would
            // spin forever on the empty intrinsic.
            while (!queue.try_consume(msg)) {
                // Yield on host; on DPU this would be WRS.NTO / WFE
                std::this_thread::yield();
            }
            uint64_t expected = TEST_PAYLOAD + i;
            if (msg != expected) {
                std::fprintf(stderr,
                    "FAIL: msg[%zu] = 0x%016lx, expected 0x%016lx\n",
                    i, msg, expected);
                consumer_ok.store(false, std::memory_order_relaxed);
            }
        }
    });

    // Producer thread
    for (size_t i = 0; i < NUM_MESSAGES; ++i) {
        queue.publish(TEST_PAYLOAD + i);
    }

    consumer.join();

    if (consumer_ok.load(std::memory_order_relaxed)) {
        std::printf("PASS: %zu messages transferred via SpscQueue\n", NUM_MESSAGES);
        return 0;
    } else {
        std::printf("FAIL: data mismatch detected\n");
        return 1;
    }
}
