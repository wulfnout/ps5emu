// Loader for the Rust core compiled to WebAssembly (crates/emulator-ffi).
// Talks to the plain C ABI — no wasm-bindgen glue — so this file is the whole
// browser/core boundary.

interface CoreExports {
  memory: WebAssembly.Memory;
  emu_result_ptr(): number;
  emu_alloc(len: number): number;
  emu_free(ptr: number, len: number): void;
  emu_core_version(): number;
  emu_run_all_tests(): number;
  emu_test_program_count(): number;
  emu_machine_new(ptr: number, len: number): number;
  emu_machine_new_test_program(index: number): number;
  emu_machine_free(h: number): void;
  emu_machine_step(h: number): number;
  emu_machine_run(h: number): number;
  emu_machine_state_json(h: number): number;
}

export const STATUS = { RUNNING: 0, HALTED: 1, ERROR: 2 } as const;

export interface Check { label: string; expected: string; actual: string; ok: boolean }
export interface ProgramReport {
  name: string;
  description: string;
  source: string;
  passed: boolean;
  steps: number;
  stop: string;
  checks: Check[];
  registers: [string, string][];
  rip: string;
  rflags: string;
  data: [string, string][];
}
export interface MachineState {
  status: number;
  retired: number;
  rip: string;
  rflags: string;
  error: string | null;
  registers: [string, string][];
  data: [string, string][];
}

const decoder = new TextDecoder();

export class Core {
  private constructor(private x: CoreExports) {}

  static async load(url: string): Promise<Core> {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`could not fetch ${url}: HTTP ${res.status}`);
    const bytes = await res.arrayBuffer();
    // The core imports nothing from the host.
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return new Core(instance.exports as unknown as CoreExports);
  }

  /** Read the core's result buffer after a call that returned its length. */
  private result(len: number): string {
    // wasm i32 returns arrive signed; >>> 0 makes pointers/lengths unsigned.
    const ptr = this.x.emu_result_ptr() >>> 0;
    len = len >>> 0;
    // Re-create the view each time: memory.buffer is replaced when memory grows.
    return decoder.decode(new Uint8Array(this.x.memory.buffer, ptr, len));
  }

  version(): string {
    return this.result(this.x.emu_core_version());
  }

  runAllTests(): ProgramReport[] {
    return JSON.parse(this.result(this.x.emu_run_all_tests()));
  }

  testProgramCount(): number {
    return this.x.emu_test_program_count();
  }

  machineForTestProgram(index: number): Machine {
    const h = this.x.emu_machine_new_test_program(index);
    if (h === 0) throw new Error(`no test program ${index}`);
    return new Machine(this, this.x, h);
  }

  machineForCode(code: Uint8Array): Machine {
    const ptr = this.x.emu_alloc(code.length) >>> 0;
    new Uint8Array(this.x.memory.buffer, ptr, code.length).set(code);
    const h = this.x.emu_machine_new(ptr, code.length);
    this.x.emu_free(ptr, code.length);
    if (h === 0) throw new Error("program does not fit in the code region");
    return new Machine(this, this.x, h);
  }

  /** @internal */
  readResult(len: number): string {
    return this.result(len);
  }
}

export class Machine {
  private freed = false;
  constructor(private core: Core, private x: CoreExports, private h: number) {}

  step(): number {
    return this.x.emu_machine_step(this.live());
  }

  run(): number {
    return this.x.emu_machine_run(this.live());
  }

  state(): MachineState {
    return JSON.parse(this.core.readResult(this.x.emu_machine_state_json(this.live())));
  }

  free(): void {
    if (!this.freed) {
      this.x.emu_machine_free(this.h);
      this.freed = true;
    }
  }

  private live(): number {
    if (this.freed) throw new Error("machine already freed");
    return this.h;
  }
}
