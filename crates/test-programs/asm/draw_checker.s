# Draws a 16-pixel checkerboard with a border, using TEST, CMOV and SAR.
    .intel_syntax noprefix
    .code64
    mov rdi, 0x100000
    mov r8d, 0x00F2F2F2     # light square
    mov r9d, 0x001B2A49     # dark square
    mov r10d, 0x00FF5D6C    # border
    xor ecx, ecx            # y
yloop:
    xor edx, edx            # x
xloop:
    mov eax, edx
    sar eax, 4
    mov ebx, ecx
    sar ebx, 4
    xor eax, ebx
    test eax, 1
    mov eax, r9d
    cmovnz eax, r8d         # odd cell -> light
    cmp edx, 4
    cmovb eax, r10d         # left border
    cmp ecx, 4
    cmovb eax, r10d         # top border
    mov [rdi], eax
    add rdi, 4
    inc edx
    cmp edx, 256
    jb xloop
    inc ecx
    cmp ecx, 144
    jb yloop
    hlt
