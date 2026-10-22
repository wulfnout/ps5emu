# Executes an instruction outside the implemented subset (CPUID).
    .intel_syntax noprefix
    .code64
    mov eax, 1
    cpuid
    hlt
