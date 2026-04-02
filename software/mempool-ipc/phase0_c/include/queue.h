#ifndef MEMPOOL_IPC_QUEUE_H
#define MEMPOOL_IPC_QUEUE_H

#include <stdint.h>
#include <stdbool.h>

// MemPool/TeraPool L1 cachelines and TCDM banks are 64-byte aligned.
// We force this alignment to physically prevent false-sharing.
#define CACHELINE_SIZE 64
#define RING_SIZE 1024

typedef struct __attribute__((aligned(CACHELINE_SIZE))) {
    uint32_t sequence;  // RVWMO Atomic sequence counter
    uint32_t data;      // AI Tensor Payload
} QueueSlot;

typedef struct {
    // The ring buffer slots, naturally padded by the struct alignment
    QueueSlot ring[RING_SIZE];
    
    // Producers and consumers track their own sequences. 
    // They are kept on separate cachelines to prevent interconnect ping-ponging.
    uint32_t next_producer_seq __attribute__((aligned(CACHELINE_SIZE)));
    uint32_t next_consumer_seq __attribute__((aligned(CACHELINE_SIZE)));
} HardwareQueue;

// RVWMO Acquire/Release atomic wrappers
static inline void publish_message(HardwareQueue* q, uint32_t seq, uint32_t msg) {
    uint32_t index = seq & (RING_SIZE - 1);
    q->ring[index].data = msg;
    // Maps to RISC-V AMO with .rl (release) semantics
    __atomic_store_n(&q->ring[index].sequence, seq, __ATOMIC_RELEASE);
}

static inline bool consume_message(HardwareQueue* q, uint32_t expected_seq, uint32_t* out_msg) {
    uint32_t index = expected_seq & (RING_SIZE - 1);
    // Maps to RISC-V AMO with .aq (acquire) semantics
    uint32_t current_seq = __atomic_load_n(&q->ring[index].sequence, __ATOMIC_ACQUIRE);
    
    if (current_seq >= expected_seq) {
        *out_msg = q->ring[index].data;
        return true;
    }
    return false;
}

#endif // MEMPOOL_IPC_QUEUE_H