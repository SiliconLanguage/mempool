// HTIF interface for Spike emulator (tohost/fromhost communication)
.section .htif, "aw", @progbits
.globl tohost
.globl fromhost
.align 6
tohost:   .word 0
          .word 0
fromhost: .word 0
          .word 0
