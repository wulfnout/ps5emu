# Phone app (Expo Go)

Shows the emulator's browser build (https://wulfnout.github.io/ps5emu/) in a
full-screen web view. Expo Go can't load the compiled Rust core directly, so
this runs the same WebAssembly core that CI tests.

Publishing is automatic: every push to `main` that touches `apps/mobile`
sends a new version to Expo Go (`.github/workflows/expo-go.yml`), once the
`EXPO_TOKEN` repository secret is set. The run's log and the Expo Go link are
saved to the `expo-go-log` branch.

The page itself updates separately: every push publishes it to GitHub Pages
(`.github/workflows/pages.yml`), and the app picks it up on the next refresh.
