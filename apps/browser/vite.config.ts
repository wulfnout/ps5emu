import { defineConfig } from "vite";

// The core is single-threaded today, so no COOP/COEP headers are required.
// When browser threads (SharedArrayBuffer) are added, set
//   Cross-Origin-Opener-Policy: same-origin
//   Cross-Origin-Embedder-Policy: require-corp
// here (server.headers / preview.headers) and on the production host.
export default defineConfig({
  base: "./",
});
