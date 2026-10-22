# Sum 1..10 with a loop, then store via a subroutine using the stack.
    .intel_syntax noprefix
    .code64
    mov rsp, 0x3000
    mov ecx, 10
    xor eax, eax
loop_top:
    add rax, rcx
    dec rcx
    jnz loop_top
    push rax
    call store
    pop rdx
    hlt
store:
    lea rdi, [rip + result_ptr]
    mov rdi, [rdi]
    mov [rdi + 8], rax
    ret
result_ptr:
    .quad 0x1000
