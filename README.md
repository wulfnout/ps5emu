# Emulator core (working title)

Your own emulator, your own code. Long-term target: PS5 software on browser
and iOS. **Current state: the first milestone of the roadmap** — a tested
x86-64 interpreter core that runs small reference programs natively, from C
(the iOS interface), and in WebAssembly (the browser). It does not load or
run PS5 games, and nothing in it pretends to. See
[docs/compatibility.md](docs/compatibility.md) for exactly what exists.

## What's here

| Path | What it is |
| --- | --- |
| `crates/guest-memory` | Guest memory: mapped regions, R/W/X permissions, precise faults |
| `crates/cpu-x64` | x86-64 decoder + interpreter for a documented subset |
| `crates/test-programs` | Reference programs (`asm/*.s`) assembled by GNU `as`, with expected results |
| `crates/emulator-core` | Wires CPU + memory, runs programs, produces reports; `emu-diag` CLI |
| `crates/emulator-ffi` | One C interface for both frontends (WebAssembly + iOS static library) |
| `apps/browser` | TypeScript/Vite diagnostics page + step debugger |
| `apps/mobile` | Expo Go phone app that shows the browser build |
| `apps/ios` | Native iOS app: not started (needs a Mac) — plan inside |
| `.github/workflows/tests.yml` | CI: native tests, C interface, WebAssembly smoke test, browser build |

## Run it

Native tests (any computer with Rust):

```sh
cargo test --workspace
cargo run -p emulator-core --bin emu-diag
```

Browser (needs Rust + Node 22):

```sh
cd apps/browser
npm install
npm run dev
```

No computer handy? Push the repo to GitHub — the `tests` workflow builds and
tests everything in the cloud, and uploads the built browser app as a
downloadable artifact.

## Milestone 1 result

The roadmap's first program — `mov rax, 7; add rax, 5; mov [rbx], rax;
mov rcx, [rbx]` — runs as real x86-64 machine bytes and ends with
`rax = rcx = [0x1000] = 12`. Four more reference programs cover loops, the
stack, CALL/RET, RIP-relative and SIB addressing, an unsupported instruction
(must stop, not skip), and a memory fault (must stop precisely).

## Next steps (roadmap Phase 3 → 4)

1. Widen the instruction set: TEST, shifts, MUL/DIV, MOVZX/MOVSX, SETcc/CMOVcc, 8/16-bit sizes.
2. Add a software framebuffer the browser draws to a `<canvas>`.
3. Page tables and virtual memory.
4. Start the iOS shell on a Mac.
