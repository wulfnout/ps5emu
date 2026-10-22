//! Command-line diagnostics runner.
//!
//!   cargo run -p emulator-core --bin emu-diag            # run every reference program
//!   cargo run -p emulator-core --bin emu-diag -- foo.bin # run a raw x86-64 binary

use emulator_core::{run_all, Machine, DEFAULT_STEP_LIMIT};
use emulator_core::cpu_x64::GPR_NAMES;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(path) = args.first() {
        let code = std::fs::read(path).unwrap_or_else(|e| {
            eprintln!("cannot read {path}: {e}");
            std::process::exit(2)
        });
        let mut m = Machine::with_standard_layout();
        if let Err(e) = m.load_code(&code) {
            eprintln!("{e}");
            std::process::exit(2);
        }
        let r = m.cpu.run(&mut m.memory, DEFAULT_STEP_LIMIT);
        println!("steps: {}  stop: {:?}", r.steps, r.stop);
        for (i, n) in GPR_NAMES.iter().enumerate() {
            println!("{n:>4} = {:#018x}", m.cpu.regs.gpr[i]);
        }
        println!(" rip = {:#018x}\nrflags = {:#x}", m.cpu.regs.rip, m.cpu.regs.rflags);
        return;
    }

    let reports = run_all();
    let mut failed = 0;
    for r in &reports {
        println!("{} {}  ({} steps, {})", if r.passed { "PASS" } else { "FAIL" }, r.name, r.steps, r.stop);
        for c in &r.checks {
            println!("    {} {:<10} expected {:<34} actual {}", if c.ok { "ok " } else { "BAD" }, c.label, c.expected, c.actual);
        }
        if !r.passed {
            failed += 1;
        }
    }
    println!("\n{} of {} reference programs passed", reports.len() - failed, reports.len());
    if failed > 0 {
        std::process::exit(1);
    }
}
