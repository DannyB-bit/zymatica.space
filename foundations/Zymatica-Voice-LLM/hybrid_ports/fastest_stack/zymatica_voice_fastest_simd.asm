; Watermark: ip zymatica.space | astronautshe.com
; Copyright (c) 2026 Zymatica. All rights reserved.
section .text
global fast_xor_simd
fast_xor_simd:
    xor rax, rax
.loop:
    cmp rax, r9
    jge .exit
    movdqa xmm0, [rcx + rax]
    pxor xmm0, [rdx + rax]
    movdqa [r8 + rax], xmm0
    add rax, 16
    jmp .loop
.exit:
    ret
