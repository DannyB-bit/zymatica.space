; ==============================================================================
; Class 33: Z-SPAR (Zymatica Semantic Parity & Repair) - x86_64 AVX2 Assembly
; Author: Danny Bouldiez | Codebase by Devs One
; License: SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
; ==============================================================================

section .data
align 16
gf16_exp_table:
    db 1, 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1
    db 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1

align 16
gf16_log_table:
    db 0, 0, 1, 4, 2, 8, 5, 10, 3, 14, 9, 7, 6, 13, 11, 12

section .text
global z_spar_gf16_mul_asm
global z_spar_gf16_add_asm

; Function: z_spar_gf16_add_asm(uint8_t a, uint8_t b) -> uint8_t
; SysV: rdi=a, rsi=b | Win64: rcx=a, rdx=b
z_spar_gf16_add_asm:
    xor cl, dl
    and cl, 0x0F
    movzx eax, cl
    ret

; Function: z_spar_gf16_mul_asm(uint8_t a, uint8_t b) -> uint8_t
z_spar_gf16_mul_asm:
    and cl, 0x0F
    and dl, 0x0F
    test cl, cl
    jz .ret_zero
    test dl, dl
    jz .ret_zero

    lea rax, [rel gf16_log_table]
    movzx r8d, byte [rax + rcx]
    movzx r9d, byte [rax + rdx]
    add r8d, r9d

    ; mod 15 reduction
    mov eax, r8d
    mov edx, 0x88888889
    mul edx
    shr edx, 3
    imul edx, 15
    sub r8d, edx

    lea rax, [rel gf16_exp_table]
    movzx eax, byte [rax + r8]
    ret

.ret_zero:
    xor eax, eax
    ret
