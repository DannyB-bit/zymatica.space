; ==============================================================================
; ZYMATICA SOVEREIGN INVENTIONS: UNIFIED MULTI-PILLAR POLYGLOT ENGINE (x86_64 Assembly)
; Author: Danny Bouldiez | Codebase by Devs One
; Classes 28-32: Epigenetic MGS Subspace Projection & Fast SIMD Dot Product
; ==============================================================================

global zymatica_mgs_dot_avx
global zymatica_octonion_conj

section .text

; float zymatica_mgs_dot_avx(const float* rdi, const float* rsi, size_t rdx)
; Computes vectorized AVX2 dot product across activation vectors
zymatica_mgs_dot_avx:
    vxorps ymm0, ymm0, ymm0      ; Accumulator = 0
    xor rax, rax

.loop:
    cmp rax, rdx
    jge .reduce
    vmovups ymm1, [rdi + rax*4]  ; Load 8 base floats
    vmovups ymm2, [rsi + rax*4]  ; Load 8 update floats
    vfmadd231ps ymm0, ymm1, ymm2 ; ymm0 += ymm1 * ymm2
    add rax, 8
    jmp .loop

.reduce:
    ; Horizontal sum of ymm0 into xmm0
    vextractf128 xmm1, ymm0, 1
    vaddps xmm0, xmm0, xmm1
    vhaddps xmm0, xmm0, xmm0
    vhaddps xmm0, xmm0, xmm0
    vzeroupper
    ret

; void zymatica_octonion_conj(const float* rdi, float* rsi)
; Conjugate an 8D Octonion in registers: [a0, -a1, -a2, -a3, -a4, -a5, -a6, -a7]
zymatica_octonion_conj:
    vmovss xmm0, [rdi]
    vmovss [rsi], xmm0           ; Real part unchanged (a0)
    vxorps xmm1, xmm1, xmm1
    mov rcx, 1
.conj_loop:
    cmp rcx, 8
    jge .done
    vmovss xmm0, [rdi + rcx*4]
    vsubss xmm2, xmm1, xmm0      ; -a[i]
    vmovss [rsi + rcx*4], xmm2
    inc rcx
    jmp .conj_loop
.done:
    ret
