# Roadmap milestone: 7 + 5 = 12, stored to guest memory and read back.
    .intel_syntax noprefix
    .code64
    mov rax, 7
    add rax, 5
    mov rbx, 0x1000
    mov [rbx], rax
    mov rcx, [rbx]
    cmp rcx, 12
    hlt
