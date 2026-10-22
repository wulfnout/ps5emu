# Controller test. Runs forever, one pass per input frame:
#   left stick  -> white square (turns green while Cross is held)
#   right stick -> blue square
#   bottom bar  -> one light per button, in DualSense order
# Reads only the controller port at 0x200000 (see docs/input.md).
    .intel_syntax noprefix
    .code64
    mov r9, 0x100000            # screen
    mov rbp, 0x200000           # controller port
    xor r12d, r12d              # previous left square x, y
    xor r13d, r13d
    xor r14d, r14d              # previous right square x, y
    xor r15d, r15d

frame:
    # erase both squares at their previous positions
    mov edi, r12d
    mov esi, r13d
    mov edx, 8
    mov ecx, 8
    xor eax, eax
    call fill
    mov edi, r14d
    mov esi, r15d
    mov edx, 8
    mov ecx, 8
    xor eax, eax
    call fill

    # new positions: x = stick*31/32 (0..247), y = stick*15/32 (0..119)
    mov eax, [rbp + 0x0C]
    imul eax, eax, 31
    shr eax, 5
    mov r12d, eax
    mov eax, [rbp + 0x10]
    imul eax, eax, 15
    shr eax, 5
    mov r13d, eax
    mov eax, [rbp + 0x14]
    imul eax, eax, 31
    shr eax, 5
    mov r14d, eax
    mov eax, [rbp + 0x18]
    imul eax, eax, 15
    shr eax, 5
    mov r15d, eax

    # right square (blue)
    mov edi, r14d
    mov esi, r15d
    mov edx, 8
    mov ecx, 8
    mov eax, 0x6EA8FF
    call fill
    # left square (white, green while Cross is held), drawn on top
    mov eax, 0xFFFFFF
    mov r10d, 0x3DDC97
    test dword ptr [rbp + 0x08], 1
    cmovnz eax, r10d
    mov edi, r12d
    mov esi, r13d
    mov edx, 8
    mov ecx, 8
    call fill

    # button bar: 18 lights, 10x6 each, at y = 136
    xor ebx, ebx
button:
    mov eax, [rbp + 0x08]
    mov ecx, ebx
    shr eax, cl
    and eax, 1
    mov r10d, 0x232A3A          # off
    mov r11d, 0x3DDC97          # on
    test eax, eax
    cmovnz r10d, r11d
    mov eax, r10d
    imul edi, ebx, 14
    add edi, 4
    mov esi, 136
    mov edx, 10
    mov ecx, 6
    call fill
    inc ebx
    cmp ebx, 18
    jb button

    # wait for the next input frame
wait:
    mov eax, [rbp]
    cmp eax, [0x1000]
    je wait
    mov [0x1000], eax
    jmp frame

# fill(edi = x, esi = y, edx = width, ecx = height, eax = color)
# clobbers r8, r10, r11, rcx
fill:
    mov r8d, esi
    shl r8d, 8
    add r8d, edi
    shl r8d, 2
    add r8, r9
    mov r10d, ecx
fill_row:
    mov rcx, r8
    mov r11d, edx
fill_col:
    mov [rcx], eax
    add rcx, 4
    dec r11d
    jnz fill_col
    add r8, 1024
    dec r10d
    jnz fill_row
    ret
