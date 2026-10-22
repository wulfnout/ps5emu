// Headless check that the WebAssembly build of the core runs the same
// reference programs as the native build and that all of them pass.
//   node apps/browser/scripts/smoke.mjs
import { readFile } from "node:fs/promises";

const url = new URL("../public/emulator_core.wasm", import.meta.url);
const { instance } = await WebAssembly.instantiate(await readFile(url), {});
const x = instance.exports;
const read = (len) =>
  new TextDecoder().decode(new Uint8Array(x.memory.buffer, x.emu_result_ptr() >>> 0, len >>> 0));

console.log(`core v${read(x.emu_core_version())}`);
const reports = JSON.parse(read(x.emu_run_all_tests()));
let failed = 0;
for (const r of reports) {
  console.log(`${r.passed ? "PASS" : "FAIL"} ${r.name} (${r.steps} steps, ${r.stop})`);
  if (!r.passed) {
    failed++;
    for (const c of r.checks.filter((c) => !c.ok)) console.log(`   ${c.label}: expected ${c.expected}, got ${c.actual}`);
  }
}

// Single-step the milestone program through the machine API, as the debugger does.
const h = x.emu_machine_new_test_program(0);
let steps = 0;
while (x.emu_machine_step(h) === 0) steps++;
const state = JSON.parse(read(x.emu_machine_state_json(h)));
x.emu_machine_free(h);
const rax = state.registers.find(([n]) => n === "rax")[1];
const mem = state.data[0][1];
console.log(`step API: ${steps + 1} instructions, rax=${rax}, [0x1000]=${mem}`);
if (rax !== "0x000000000000000c" || mem !== "0x000000000000000c") failed++;

console.log(failed ? `${failed} failure(s)` : `all ${reports.length} reference programs passed in WebAssembly`);
process.exit(failed ? 1 : 0);
