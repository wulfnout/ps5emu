# Compatibility and status

Honest status of every subsystem. "Implemented" means covered by a test that
runs in CI. Nothing here boots PS5 software.

## CPU (crates/cpu-x64)

| Area | Status |
| --- | --- |
| Register file (16 GPRs, RIP, RFLAGS) | Implemented |
| 64-bit and 32-bit operand sizes (32-bit writes zero-extend) | Implemented |
| 8/16-bit operand sizes, `66` prefix | **Not implemented** — reported as unsupported |
| ADD OR AND SUB XOR CMP (reg/mem/imm forms) | Implemented, results + flags checked against real x86-64 hardware |
| ADC SBB | **Not implemented** |
| Accumulator short forms (05 0D 25 2D 35 3D A9) | Implemented |
| TEST, NOT, NEG | Implemented, checked against hardware |
| SHL/SHR/SAR (by 1, CL, imm8) | Implemented, checked against hardware |
| IMUL (two- and three-operand) | Implemented, checked against hardware |
| CMOVcc (incl. 32-bit zero-extend quirk) | Implemented, checked against hardware |
| INC DEC (CF preserved) | Implemented, checked against hardware |
| MOV (89, 8B, C7 /0, B8+r incl. 64-bit immediate), LEA | Implemented |
| Jcc (all 16 conditions, rel8/rel32), JMP rel8/rel32, CALL rel32, RET | Implemented |
| PUSH/POP r64 | Implemented |
| NOP, HLT | Implemented |
| ModRM/SIB addressing, RIP-relative | Implemented |
| Rotates, MUL/DIV (one-operand), MOVZX/MOVSX, SETcc | **Not implemented** |
| String ops, LOCK, REP | **Not implemented** |
| SSE / AVX / AVX2 (VEX), x87 | **Not implemented** — Zen 2 games rely heavily on these |
| CPUID, RDTSC, SYSCALL, system instructions | **Not implemented** |
| Interrupts/exceptions delivered to guest handlers | **Not implemented** — faults stop the emulator precisely (RIP stays on the faulting instruction) |
| JIT / dynamic recompiler | **Not started** |

Any instruction outside this list stops execution with
`unsupported instruction (<bytes>)` and the exact RIP. Nothing is skipped.

## Memory (crates/guest-memory)

| Area | Status |
| --- | --- |
| Mapped regions with R/W/X permissions, bounds checks, faults | Implemented |
| Accesses spanning two regions | Fault (stricter than hardware) |
| Page tables / virtual address translation | **Not implemented** |
| Screen: 256×144 32-bit framebuffer at 0x100000 | Implemented (simple test screen, not the PS5 GPU) |
| Other memory-mapped devices | **Not implemented** |

## Console systems

| Area | Status |
| --- | --- |
| Executable loading (PS5 formats) | **Not started** |
| Kernel / OS services (threads, sync, files, networking) | **Not started** |
| GPU command processing, shader translation | **Not started** |
| Audio | **Not started** |
| Controller input | Controller port at 0x200000 fed by DualSense/other controllers and keyboard + mouse (docs/input.md). PS5 system pad API not implemented |

## Frontends

| Area | Status |
| --- | --- |
| Native CLI diagnostics (`emu-diag`) | Working |
| C interface for iOS (`include/emulator.h` + static library) | Working, tested from C in CI |
| Browser: WebAssembly build + diagnostics/debugger page | Code complete; built and tested in CI (see README) |
| iOS SwiftUI app | **Not started** — needs a Mac with Xcode |
