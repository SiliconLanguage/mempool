#ifndef SL_INTRINSICS_H
#define SL_INTRINSICS_H

#include <cstdint>

// --- Memory Locality Hints (Non-Temporal) ---
inline void sl_load_nt(uint64_t* addr1, uint64_t* addr2, uint64_t& r1, uint64_t& r2) {
#if defined(__riscv)
    // RISC-V: Apply NTL.PALL (add x0, x0, x3) immediately before EACH load
    asm volatile(
        "add x0, x0, x3 \n\t"
        "ld %0, 0(%2) \n\t"
        "add x0, x0, x3 \n\t"
        "ld %1, 0(%3) \n\t"
        : "=r"(r1), "=r"(r2) 
        : "r"(addr1), "r"(addr2) 
        : "memory"
    );
#elif defined(__aarch64__)
    // ARM64: Use Load Non-Temporal Pair (LDNP)
    asm volatile(
        "ldnp %0, %1, [%2]"
        : "=r"(r1), "=r"(r2) 
        : "r"(addr1) 
        : "memory"
    );
#endif
}

// --- Hardware-Assisted Polling (Wait-on-Reservation) ---
inline void sl_wait_on_address(volatile uint64_t* addr, uint64_t expected_val) {
    uint64_t temp;
#if defined(__riscv)
    // RISC-V: Zawrs WRS.NTO with RVWMO Acquire semantics (.aq)
    asm volatile(
        "1: lr.d.aq %0, (%1) \n\t"
        "beq %0, %2, 2f \n\t"
        "wrs.nto \n\t"
        "j 1b \n\t"
        "2:"
        : "=&r"(temp) 
        : "r"(addr), "r"(expected_val) 
        : "memory"
    );
#elif defined(__aarch64__)
    // ARM64: WFE with Load-Acquire Exclusive (LDAXR)
    asm volatile(
        "1: ldaxr %0, [%1] \n\t"
        "cmp %0, %2 \n\t"
        "b.eq 2f \n\t"
        "wfe \n\t"
        "b 1b \n\t"
        "2:"
        : "=&r"(temp) 
        : "r"(addr), "r"(expected_val) 
        : "cc", "memory"
    );
#endif
}

#endif
