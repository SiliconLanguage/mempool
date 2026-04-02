#include <stdint.h>
#include <stdbool.h>
#include "runtime.h" // Provides mempool_get_core_id()
#include "../include/queue.h"

// Allocate the queue and our custom barrier in the shared L1 TCDM
volatile HardwareQueue ipc_queue __attribute__((section(".l1")));
volatile uint32_t boot_barrier __attribute__((section(".l1"))) = 0;

// Our own bare-metal spin-barrier
static inline void hw_barrier(uint32_t num_cores) {
    __atomic_fetch_add(&boot_barrier, 1, __ATOMIC_SEQ_CST);
    while (boot_barrier < num_cores) {
        __asm__ volatile ("nop");
    }
}

// Our own bare-metal cycle wait
static inline void hw_wait(uint32_t cycles) {
    for (volatile uint32_t i = 0; i < cycles; i++) {
        __asm__ volatile ("nop");
    }
}

int main() {
    uint32_t core_id = mempool_get_core_id();
    uint32_t num_cores = mempool_get_core_count();

    // Core 0 Initializes the Queue
    if (core_id == 0) {
        for (int i = 0; i < RING_SIZE; i++) {
            ipc_queue.ring[i].sequence = 0;
            ipc_queue.ring[i].data = 0;
        }
        ipc_queue.next_producer_seq = 1;
        ipc_queue.next_consumer_seq = 1;
    }

    // Wait for L1 initialization
    hw_barrier(num_cores);

    // --- SPSC DATA PLANE TEST ---
    if (core_id == 0) {
        uint32_t payload = 0xDEADBEEF;
        uint32_t seq = ipc_queue.next_producer_seq++;
        
        hw_wait(100); // Guarantee consumer is waiting
        publish_message((HardwareQueue*)&ipc_queue, seq, payload);

    } else if (core_id == 1) {
        uint32_t expected_seq = ipc_queue.next_consumer_seq++;
        uint32_t received_msg = 0;

        while (!consume_message((HardwareQueue*)&ipc_queue, expected_seq, &received_msg)) {
            __asm__ volatile ("nop");
        }

        if (received_msg != 0xDEADBEEF) {
            return -1; 
        }
    }

    hw_barrier(num_cores);
    return 0;
}