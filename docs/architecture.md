# Architecture

```
apps/browser (TypeScript, Vite)        apps/ios (Swift, SwiftUI, Metal) — not started
        │  WebAssembly exports                  │  include/emulator.h (static library)
        └──────────────┬────────────────────────┘
                 crates/emulator-ffi      one C ABI for both platforms
                       │
                 crates/emulator-core     machine assembly, program runner, reports
                  ├── crates/cpu-x64      decoder + interpreter
                  ├── crates/guest-memory regions, permissions, faults
                  └── crates/test-programs reference programs (GNU as output) + expectations
```

Rules that keep the core portable:

- Dependencies point downward only. `cpu-x64` knows nothing about browsers,
  SwiftUI, GPUs or JSON.
- The core crates have **zero third-party dependencies**.
- Frontends get one boundary: `emulator-ffi`. The browser does not use
  wasm-bindgen; it calls the same C-ABI functions iOS calls, so both
  platforms always exercise identical code paths.
- 64-bit values cross the boundary as hex strings (JavaScript numbers lose
  precision above 2^53).
- Errors are values. A guest fault or unsupported instruction is reported
  with RIP and bytes; the host never panics on guest behavior.

## How correctness is checked

1. **Unit tests** per crate (decoder encodings, memory faults, etc.).
2. **Hardware differential tests** (`cpu-x64/tests/hardware_reference.rs`):
   on an x86-64 test machine, each ALU operation is executed on the real
   processor with inline assembly, and our result and flags must match
   bit-for-bit. ~25k operation pairs per run.
3. **Reference programs**: written in assembly, assembled by GNU `as`, run
   by the emulator, checked against expected registers/memory/stop reason.
   CI re-assembles them and fails if the committed bytes drift.
4. **Same tests in every build**: native, C (static library), and
   WebAssembly (Node smoke test) all run the reference programs.
