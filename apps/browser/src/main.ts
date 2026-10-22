import "./style.css";
import { Core, Machine, STATUS, type MachineState, type ProgramReport } from "./core";

// Change this to your product name; it is the only place the UI names itself.
const PRODUCT_NAME = "Emulator Core";

const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;
const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

document.title = `${PRODUCT_NAME} — Diagnostics`;
$("#product").textContent = PRODUCT_NAME;

let core: Core;
let reports: ProgramReport[] = [];
let machine: Machine | null = null;
let selected = 0;

async function boot() {
  const status = $("#core-status");
  try {
    core = await Core.load(`${import.meta.env.BASE_URL}emulator_core.wasm`);
  } catch (e) {
    status.className = "pill bad";
    status.textContent = "core not loaded";
    $("#results").innerHTML = `<div class="error-box">
      <strong>The WebAssembly core could not be loaded.</strong>
      <p>${esc(String(e))}</p>
      <p>Build it first: <code>npm run build:wasm</code> (needs the <code>wasm32-unknown-unknown</code> Rust target).</p>
    </div>`;
    return;
  }
  status.className = "pill ok";
  status.textContent = `core v${core.version()} · WebAssembly`;
  runTests();
  setupScreen();
  buildPicker();
  // Open on the first program that draws, and play it.
  selected = Math.max(0, reports.findIndex((r) => r.draws));
  ($("#program") as HTMLSelectElement).value = String(selected);
  resetMachine();
  play();
}

function runTests() {
  const t0 = performance.now();
  reports = core.runAllTests();
  const ms = performance.now() - t0;
  const passed = reports.filter((r) => r.passed).length;
  $("#summary").innerHTML = `<span class="${passed === reports.length ? "ok" : "bad"}">${passed} / ${reports.length}</span>
    reference programs passed <span class="dim">in ${ms.toFixed(1)} ms, inside your browser</span>`;
  $("#results").innerHTML = reports
    .map(
      (r, i) => `<article class="card ${r.passed ? "" : "failed"}">
      <header>
        <span class="badge ${r.passed ? "ok" : "bad"}">${r.passed ? "PASS" : "FAIL"}</span>
        <h3>${esc(r.name)}</h3>
        <button class="link" data-open="${i}">debug →</button>
      </header>
      <p>${esc(r.description)}</p>
      <p class="dim mono">${r.steps} instructions · ${esc(r.stop)}</p>
      <table>
        <thead><tr><th></th><th>check</th><th>expected</th><th>actual</th></tr></thead>
        <tbody>${r.checks
          .map(
            (c) => `<tr class="${c.ok ? "" : "bad-row"}"><td>${c.ok ? "✓" : "✗"}</td><td class="mono">${esc(c.label)}</td>
            <td class="mono">${esc(c.expected)}</td><td class="mono">${esc(c.actual)}</td></tr>`,
          )
          .join("")}</tbody>
      </table>
    </article>`,
    )
    .join("");
  document.querySelectorAll<HTMLButtonElement>("[data-open]").forEach((b) =>
    b.addEventListener("click", () => {
      selected = Number(b.dataset.open);
      ($("#program") as HTMLSelectElement).value = String(selected);
      resetMachine();
      $("#debugger").scrollIntoView({ behavior: "smooth" });
      if (reports[selected].draws) play();
    }),
  );
}

function buildPicker() {
  const sel = $("#program") as HTMLSelectElement;
  sel.innerHTML = reports.map((r, i) => `<option value="${i}">${esc(r.name)}${r.draws ? " 🖼" : ""}</option>`).join("");
  sel.addEventListener("change", () => {
    selected = Number(sel.value);
    resetMachine();
    if (reports[selected].draws) play();
  });
  $("#btn-step").addEventListener("click", () => act((m) => m.step()));
  $("#btn-run").addEventListener("click", play);
  $("#btn-reset").addEventListener("click", resetMachine);
  $("#btn-rerun").addEventListener("click", runTests);
}

let previous: MachineState | null = null;
let playing = 0;
let screenCtx: CanvasRenderingContext2D | null = null;
let screenImg: ImageData | null = null;

function setupScreen() {
  const canvas = $("#screen") as HTMLCanvasElement;
  const { width, height } = core.screenSize();
  canvas.width = width;
  canvas.height = height;
  screenCtx = canvas.getContext("2d");
  screenImg = screenCtx?.createImageData(width, height) ?? null;
}

function drawScreen() {
  if (!machine || !screenCtx || !screenImg) return;
  machine.drawScreen(screenImg);
  screenCtx.putImageData(screenImg, 0, 0);
}

/** Run a slice of instructions per animation frame so the drawing is visible. */
function play() {
  if (!machine || playing) return;
  previous = null;
  const tick = () => {
    if (!machine) return;
    const status = machine.runSteps(12000);
    render();
    playing = status === STATUS.RUNNING ? requestAnimationFrame(tick) : 0;
  };
  playing = requestAnimationFrame(tick);
}

function stop() {
  if (playing) cancelAnimationFrame(playing);
  playing = 0;
}

function resetMachine() {
  stop();
  machine?.free();
  machine = core.machineForTestProgram(selected);
  previous = null;
  $("#source").textContent = reports[selected]?.source ?? "";
  render();
}

function act(f: (m: Machine) => number) {
  if (!machine) return;
  stop();
  previous = machine.state();
  f(machine);
  render();
}

function render() {
  if (!machine) return;
  const s = machine.state();
  const label = s.status === STATUS.RUNNING ? "running" : s.status === STATUS.HALTED ? "halted" : "stopped on error";
  $("#m-status").innerHTML = `<span class="badge ${s.status === STATUS.ERROR ? "bad" : s.status === STATUS.HALTED ? "ok" : "run"}">${label}</span>
    <span class="mono">rip ${s.rip}</span> <span class="mono">rflags ${s.rflags}</span>
    <span class="dim">${s.retired} retired</span>`;
  $("#m-error").textContent = s.error ?? "";
  ($("#btn-step") as HTMLButtonElement).disabled = s.status !== STATUS.RUNNING;
  ($("#btn-run") as HTMLButtonElement).disabled = s.status !== STATUS.RUNNING || playing !== 0;
  drawScreen();

  const changed = (i: number, list: "registers" | "data") =>
    previous && previous[list][i][1] !== s[list][i][1] ? "changed" : "";
  $("#regs").innerHTML = s.registers
    .map(([n, v], i) => `<div class="cell ${changed(i, "registers")}"><span>${n}</span><code>${v}</code></div>`)
    .join("");
  $("#data").innerHTML = s.data
    .map(([a, v], i) => `<div class="cell ${changed(i, "data")}"><span>${a}</span><code>${v}</code></div>`)
    .join("");
}

boot();
