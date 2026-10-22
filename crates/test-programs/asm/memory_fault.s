# Writes to an unmapped address; must fault precisely.
    .intel_syntax noprefix
    .code64
    mov rax, 42
    mov rbx, 0xDEAD0000
    mov [rbx], rax
    hlt
