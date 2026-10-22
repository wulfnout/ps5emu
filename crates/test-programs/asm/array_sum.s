# Sum a 4-element array with SIB addressing [base + index*8]; signed compare loop.
    .intel_syntax noprefix
    .code64
    mov rsi, 0x1000
    mov qword ptr [rsi], 100
    mov qword ptr [rsi + 8], -30
    mov qword ptr [rsi + 16], 7
    mov qword ptr [rsi + 24], 0x7FFFFFFF
    xor ebx, ebx
    xor eax, eax
next:
    add rax, [rsi + rbx*8]
    inc rbx
    cmp rbx, 4
    jl next
    mov [rsi + 32], rax
    hlt
