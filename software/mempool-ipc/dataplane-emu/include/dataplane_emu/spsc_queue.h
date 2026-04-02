#ifndef SPSC_QUEUE_H
#define SPSC_QUEUE_H

#include <atomic>
#include <cstdint>
#include <cstddef>
#include "sl_intrinsics.h"

namespace dataplane_emu {

// Cacheline size matching MemPool L1 TCDM bank width
static constexpr size_t CACHELINE_SIZE = 64;

/// A single slot in the lock-free ring buffer.
/// Alignment to CACHELINE_SIZE prevents false sharing on the TCDM interconnect.
struct alignas(CACHELINE_SIZE) QueueSlot {
    std::atomic<uint64_t> sequence;
    uint64_t data;
};

/// SPSC lock-free ring buffer with hardware-assisted polling.
///
/// Uses sl_wait_on_address() for zero-traffic consumer polling and
/// sl_load_nt() for non-temporal data fetches that avoid cache pollution.
///
/// Template parameter N must be a power of 2.
template <size_t N = 1024>
class SpscQueue {
    static_assert((N & (N - 1)) == 0, "Ring size must be a power of 2");

public:
    SpscQueue() {
        for (size_t i = 0; i < N; ++i) {
            ring_[i].sequence.store(0, std::memory_order_relaxed);
            ring_[i].data = 0;
        }
        next_producer_seq_ = 1;
        next_consumer_seq_ = 1;
    }

    /// Publish a message with release semantics.
    /// Called by the single producer only.
    void publish(uint64_t msg) {
        uint64_t seq = next_producer_seq_++;
        size_t index = seq & (N - 1);
        ring_[index].data = msg;
        ring_[index].sequence.store(seq, std::memory_order_release);
    }

    /// Consume a message using hardware-assisted polling.
    /// Spins via sl_wait_on_address (LR.D.AQ + WRS.NTO on RISC-V,
    /// LDAXR + WFE on ARM64) until the slot's sequence matches.
    /// Returns the payload with acquire ordering guaranteed by the intrinsic.
    uint64_t consume() {
        uint64_t seq = next_consumer_seq_++;
        size_t index = seq & (N - 1);

        // Hardware-assisted poll: suspend core until sequence == seq
        auto* seq_addr = reinterpret_cast<volatile uint64_t*>(&ring_[index].sequence);
        sl_wait_on_address(seq_addr, seq);

        // Non-temporal load of the payload to avoid cache pollution.
        // Both addresses point into the same cacheline-aligned slot,
        // so the pair load fetches (sequence, data) without displacing
        // hot working-set lines.
        uint64_t nt_seq, nt_data;
        sl_load_nt(
            reinterpret_cast<uint64_t*>(&ring_[index].sequence),
            reinterpret_cast<uint64_t*>(&ring_[index].data),
            nt_seq, nt_data
        );

        return nt_data;
    }

    /// Non-blocking consume attempt. Returns true if a message was available.
    bool try_consume(uint64_t& out_msg) {
        uint64_t seq = next_consumer_seq_;
        size_t index = seq & (N - 1);

        uint64_t current = ring_[index].sequence.load(std::memory_order_acquire);
        if (current < seq) {
            return false;
        }

        out_msg = ring_[index].data;
        ++next_consumer_seq_;
        return true;
    }

private:
    QueueSlot ring_[N];
    alignas(CACHELINE_SIZE) uint64_t next_producer_seq_;
    alignas(CACHELINE_SIZE) uint64_t next_consumer_seq_;
};

} // namespace dataplane_emu

#endif // SPSC_QUEUE_H
