# Draws a 256x144 color gradient into the framebuffer at 0x100000.
# Each pixel is 0x00RRGGBB: red = x, green = (y*7)>>2, blue = (x ^ y) & 0xFF.
    .intel_syntax noprefix
    .code64
    mov rdi, 0x100000
    xor ecx, ecx            # y
yloop:
    xor edx, edx            # x
xloop:
    mov eax, edx
    shl eax, 16             # red
    imul ebx, ecx, 7
    shr ebx, 2
    shl ebx, 8              # green
    or eax, ebx
    mov ebx, edx
    xor ebx, ecx
    and ebx, 0xFF           # blue
    or eax, ebx
    mov [rdi], eax
    add rdi, 4
    inc edx
    cmp edx, 256
    jb xloop
    inc ecx
    cmp ecx, 144
    jb yloop
    hlt
