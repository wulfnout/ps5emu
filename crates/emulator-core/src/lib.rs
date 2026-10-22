//! Emulator core: wires the CPU to guest memory and runs programs.
//!
//! Platform-independent. Has no knowledge of browsers, SwiftUI or GPUs — the
//! frontends call into this crate, never the other way round.
//!
//! What this is today: an x86-64 integer-core interpreter running small
//! reference programs. What it is NOT yet: anything that loads PS5 software.
//! There is no kernel/OS service layer, no GPU, no audio, no file system.

use cpu_x64::{flags, Cpu, CpuError, StopReason, GPR_NAMES};
use guest_memory::{GuestMemory, Perms};
use test_programs::{ExpectedStop, TestProgram};
use test_programs::{CODE_BASE, CODE_SIZE, DATA_BASE, DATA_SIZE, FB_BASE, FB_HEIGHT, FB_SIZE, FB_WIDTH, STACK_BASE, STACK_SIZE};

pub use cpu_x64;
pub use guest_memory;
pub use test_programs;

/// Hard cap so a runaway guest loop can never hang a frontend.
pub const DEFAULT_STEP_LIMIT: u64 = 1_000_000;

pub struct Machine {
    pub cpu: Cpu,
    pub memory: GuestMemory,
}

impl Machine {
    /// Standard test layout: code (RX), data (RW), stack (RW).
    pub fn with_standard_layout() -> Self {
        let mut memory = GuestMemory::new();
        memory.map(CODE_BASE, CODE_SIZE, Perms::RX).expect("code map");
        memory.map(DATA_BASE, DATA_SIZE, Perms::RW).expect("data map");
        memory.map(STACK_BASE, STACK_SIZE, Perms::RW).expect("stack map");
        memory.map(FB_BASE, FB_SIZE, Perms::RW).expect("screen map");
        let mut cpu = Cpu::new();
        cpu.regs.gpr[4] = STACK_BASE + STACK_SIZE; // rsp
        Self { cpu, memory }
    }

    /// The screen as raw 0x00RRGGBB little-endian pixels, row by row.
    pub fn framebuffer(&self) -> &[u8] {
        self.memory.view(FB_BASE, FB_SIZE as usize).expect("screen is mapped")
    }

    pub const SCREEN_WIDTH: u32 = FB_WIDTH;
    pub const SCREEN_HEIGHT: u32 = FB_HEIGHT;

    pub fn load_code(&mut self, code: &[u8]) -> Result<(), String> {
        if code.len() as u64 > CODE_SIZE {
            return Err(format!("program is {} bytes; code region is {CODE_SIZE}", code.len()));
        }
        self.memory.load(CODE_BASE, code).map_err(|e| e.to_string())?;
        self.cpu.regs.rip = CODE_BASE;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Check {
    pub label: String,
    pub expected: String,
    pub actual: String,
    pub ok: bool,
}

#[derive(Debug, Clone)]
pub struct ProgramReport {
    pub name: String,
    pub description: String,
    pub source: String,
    pub passed: bool,
    pub steps: u64,
    pub stop: String,
    pub checks: Vec<Check>,
    pub registers: Vec<(String, u64)>,
    pub rip: u64,
    pub rflags: u64,
    pub data_preview: Vec<(u64, u64)>,
    pub draws: bool,
}

fn check(checks: &mut Vec<Check>, label: String, expected: String, actual: String) {
    let ok = expected == actual;
    checks.push(Check { label, expected, actual, ok });
}

fn describe_stop(stop: &StopReason) -> String {
    match stop {
        StopReason::Halted => "halted (HLT)".into(),
        StopReason::StepLimit => "step limit reached".into(),
        StopReason::Error(e) => e.to_string(),
    }
}

pub fn run_test_program(p: &TestProgram) -> ProgramReport {
    let mut m = Machine::with_standard_layout();
    let mut checks = Vec::new();
    let loaded = m.load_code(p.code);
    let result = m.cpu.run(&mut m.memory, DEFAULT_STEP_LIMIT);

    if let Err(e) = loaded {
        check(&mut checks, "load".into(), "ok".into(), e);
    }

    // Stop reason
    let (exp_stop, act_stop) = match (p.expect_stop, &result.stop) {
        (ExpectedStop::Halted, s) => ("halted".to_string(), short_stop(s)),
        (ExpectedStop::Unsupported { rip }, s) => (format!("unsupported @ {rip:#x}"), short_stop(s)),
        (ExpectedStop::MemoryFault { rip, addr }, s) => (format!("memory fault @ {rip:#x} addr {addr:#x}"), short_stop(s)),
    };
    check(&mut checks, "stop".into(), exp_stop, act_stop);

    for &(r, v) in p.expect_regs {
        check(&mut checks, GPR_NAMES[r as usize].into(), format!("{v:#x}"), format!("{:#x}", m.cpu.regs.gpr[r as usize]));
    }
    for &(addr, v) in p.expect_mem {
        let actual = m.memory.read_u64(addr).map(|x| format!("{x:#x}")).unwrap_or_else(|e| e.to_string());
        check(&mut checks, format!("[{addr:#x}]"), format!("{v:#x}"), actual);
    }
    for &(addr, v) in p.expect_mem32 {
        let actual = m.memory.read_u32(addr).map(|x| format!("{x:#08x}")).unwrap_or_else(|e| e.to_string());
        check(&mut checks, format!("px[{addr:#x}]"), format!("{v:#08x}"), actual);
    }
    if let Some(zf) = p.expect_zf {
        check(&mut checks, "ZF".into(), zf.to_string(), m.cpu.regs.flag(flags::ZF).to_string());
    }

    let data_preview = (0..8)
        .map(|i| {
            let a = DATA_BASE + i * 8;
            (a, m.memory.read_u64(a).unwrap_or(0))
        })
        .collect();

    ProgramReport {
        name: p.name.into(),
        description: p.description.into(),
        source: p.source.into(),
        passed: checks.iter().all(|c| c.ok),
        steps: result.steps,
        stop: describe_stop(&result.stop),
        checks,
        registers: GPR_NAMES.iter().zip(m.cpu.regs.gpr).map(|(n, v)| (n.to_string(), v)).collect(),
        rip: m.cpu.regs.rip,
        rflags: m.cpu.regs.rflags,
        data_preview,
        draws: p.draws,
    }
}

fn short_stop(s: &StopReason) -> String {
    match s {
        StopReason::Halted => "halted".into(),
        StopReason::StepLimit => "step limit".into(),
        StopReason::Error(CpuError::Decode { rip, .. }) => format!("unsupported @ {rip:#x}"),
        StopReason::Error(CpuError::Memory { rip, fault }) => format!("memory fault @ {rip:#x} addr {:#x}", fault.addr),
        StopReason::Error(CpuError::Halted) => "halted".into(),
    }
}

pub fn run_all() -> Vec<ProgramReport> {
    test_programs::ALL.iter().map(run_test_program).collect()
}

// ---------- JSON (hand-written to keep the core dependency-free) ----------

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

impl ProgramReport {
    /// 64-bit values are emitted as hex strings: JSON numbers lose precision above 2^53.
    pub fn to_json(&self) -> String {
        let checks: Vec<String> = self
            .checks
            .iter()
            .map(|c| format!(r#"{{"label":{},"expected":{},"actual":{},"ok":{}}}"#, esc(&c.label), esc(&c.expected), esc(&c.actual), c.ok))
            .collect();
        let regs: Vec<String> = self.registers.iter().map(|(n, v)| format!(r#"[{},"{v:#018x}"]"#, esc(n))).collect();
        let data: Vec<String> = self.data_preview.iter().map(|(a, v)| format!(r#"["{a:#06x}","{v:#018x}"]"#)).collect();
        format!(
            r#"{{"name":{},"description":{},"source":{},"passed":{},"steps":{},"stop":{},"checks":[{}],"registers":[{}],"rip":"{:#x}","rflags":"{:#x}","data":[{}],"draws":{}}}"#,
            esc(&self.name), esc(&self.description), esc(&self.source), self.passed, self.steps, esc(&self.stop),
            checks.join(","), regs.join(","), self.rip, self.rflags, data.join(","), self.draws
        )
    }
}

pub fn run_all_json() -> String {
    let parts: Vec<String> = run_all().iter().map(|r| r.to_json()).collect();
    format!("[{}]", parts.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reference_program_passes() {
        for r in run_all() {
            for c in &r.checks {
                assert!(c.ok, "{}: {} expected {} got {}", r.name, c.label, c.expected, c.actual);
            }
            assert!(r.passed, "{}", r.name);
        }
    }

    #[test]
    fn json_is_well_formed_enough() {
        let j = run_all_json();
        assert!(j.starts_with('[') && j.ends_with(']'));
        assert!(j.contains(r#""name":"add_and_store""#));
    }

    #[test]
    fn runaway_loop_hits_step_limit_not_hang() {
        let mut m = Machine::with_standard_layout();
        m.load_code(&[0xEB, 0xFE]).unwrap(); // jmp $
        let r = m.cpu.run(&mut m.memory, 1000);
        assert_eq!(r.stop, StopReason::StepLimit);
        assert_eq!(r.steps, 1000);
    }

    #[test]
    fn stores_into_code_region_fault() {
        // mov [rip+0], eax  -> 89 05 00 00 00 00 ; code region is read+execute only
        let mut m = Machine::with_standard_layout();
        m.load_code(&[0x89, 0x05, 0, 0, 0, 0, 0xF4]).unwrap();
        let r = m.cpu.run(&mut m.memory, 10);
        assert!(matches!(r.stop, StopReason::Error(CpuError::Memory { rip: 0, .. })));
    }
}
