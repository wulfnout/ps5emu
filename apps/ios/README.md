# iOS app — not started

Per the roadmap, the iOS shell comes after the shared core works. The core
side is ready:

- `crates/emulator-ffi` builds a `staticlib` (`libemulator_ffi.a`).
- `crates/emulator-ffi/include/emulator.h` is the interface Swift will import.
- CI compiles a C program against that header and library and runs the
  reference programs through it, so the interface is tested before any
  Swift exists.

Planned steps (need a Mac with Xcode):

1. `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`
2. Build the static library for both targets and wrap them in an
   `.xcframework` (`xcodebuild -create-xcframework`).
3. Create a SwiftUI app, add the xcframework and a module map for
   `emulator.h`.
4. Screens: diagnostics (same reference-program report as the browser),
   step debugger, then library and controller input (GameController).
5. Metal renderer once the core produces frames.
