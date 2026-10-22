//! Differential test against the real CPU.
//!
//! When the test machine is itself x86-64, we execute the same ALU instruction
//! on the host processor (inline asm) and require our interpreter to produce
//! the identical result and status flags. This is an independent reference,
//! not a re-statement of our own formulas.

#![cfg(target_arch = "x86_64")]

use cpu_x64::decoder::{AluOp, Width};
use cpu_x64::flags;
use cpu_x64::interpreter::alu;
use std::arch::asm;

macro_rules! hw64 {
    ($insn:literal, $a:expr, $b:expr) => {{
        let mut r: u64 = $a;
        let fl: u64;
        unsafe {
            asm!(concat!($insn, " {r}, {b}"), "pushfq", "pop {f}",
                 r = inout(reg) r, b = in(reg) $b, f = out(reg) fl);
        }
        (r, fl)
    }};
}

macro_rules! hw32 {
    ($insn:literal, $a:expr, $b:expr) => {{
        let mut r: u64 = $a;
        let fl: u64;
        unsafe {
            asm!(concat!($insn, " {r:e}, {b:e}"), "pushfq", "pop {f}",
                 r = inout(reg) r, b = in(reg) $b, f = out(reg) fl);
        }
        (r & 0xFFFF_FFFF, fl)
    }};
}

macro_rules! hw_unary {
    ($insn:literal, $suffix:literal, $a:expr, $cf_in:expr) => {{
        let mut r: u64 = $a;
        let fl: u64;
        unsafe {
            // set CF to a known value first, so we can check INC/DEC preserve it
            if $cf_in { asm!("stc") } else { asm!("clc") }
            asm!(concat!($insn, " {r", $suffix, "}"), "pushfq", "pop {f}",
                 r = inout(reg) r, f = out(reg) fl);
        }
        (r, fl)
    }};
}

fn operands() -> Vec<u64> {
    let mut v = vec![
        0, 1, 2, 0xF, 0x10, 0x7F, 0x80, 0xFF, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF,
        0x1_0000_0000, 0x7FFF_FFFF_FFFF_FFFF, 0x8000_0000_0000_0000, u64::MAX, u64::MAX - 1,
    ];
    let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..48 {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        v.push(s);
    }
    v
}

fn check(op: AluOp, width: Width, a: u64, b: u64, hw: (u64, u64)) {
    let (r, f) = alu(op, width, a, b);
    // AF is undefined for AND/OR/XOR, so it is excluded from comparison there.
    let cmp_mask = match op {
        AluOp::And | AluOp::Or | AluOp::Xor => flags::STATUS & !flags::AF,
        _ => flags::STATUS,
    };
    // CMP discards its result, so only the flags are compared for it.
    if op != AluOp::Cmp {
        assert_eq!(r, hw.0, "{op:?} {width:?} result a={a:#x} b={b:#x}");
    }
    assert_eq!(
        f & cmp_mask,
        hw.1 & cmp_mask,
        "{op:?} {width:?} flags a={a:#x} b={b:#x} ours={f:#06x} hw={:#06x}",
        hw.1
    );
}

#[test]
fn alu_64bit_matches_hardware() {
    let ops = operands();
    let mut n = 0;
    for &a in &ops {
        for &b in &ops {
            check(AluOp::Add, Width::W64, a, b, hw64!("add", a, b));
            check(AluOp::Sub, Width::W64, a, b, hw64!("sub", a, b));
            check(AluOp::Cmp, Width::W64, a, b, hw64!("cmp", a, b));
            check(AluOp::And, Width::W64, a, b, hw64!("and", a, b));
            check(AluOp::Or, Width::W64, a, b, hw64!("or", a, b));
            check(AluOp::Xor, Width::W64, a, b, hw64!("xor", a, b));
            n += 6;
        }
    }
    eprintln!("compared {n} 64-bit ALU operations against the host CPU");
}

#[test]
fn alu_32bit_matches_hardware() {
    let ops = operands();
    for &a in &ops {
        for &b in &ops {
            check(AluOp::Add, Width::W32, a, b, hw32!("add", a, b));
            check(AluOp::Sub, Width::W32, a, b, hw32!("sub", a, b));
            check(AluOp::Cmp, Width::W32, a, b, hw32!("cmp", a, b));
            check(AluOp::And, Width::W32, a, b, hw32!("and", a, b));
            check(AluOp::Or, Width::W32, a, b, hw32!("or", a, b));
            check(AluOp::Xor, Width::W32, a, b, hw32!("xor", a, b));
        }
    }
}

/// Full-instruction check for INC/DEC through the interpreter, including that
/// CF is preserved (a classic emulator bug).
#[test]
fn inc_dec_match_hardware_and_preserve_cf() {
    use cpu_x64::Cpu;
    use guest_memory::{GuestMemory, Perms};

    // 48 FF C0 = inc rax ; 48 FF C8 = dec rax ; F4 = hlt
    for (code, insn) in [([0x48u8, 0xFF, 0xC0, 0xF4], "inc"), ([0x48, 0xFF, 0xC8, 0xF4], "dec")] {
        for &a in &operands() {
            for cf_in in [false, true] {
                let hw = if insn == "inc" {
                    hw_unary!("inc", "", a, cf_in)
                } else {
                    hw_unary!("dec", "", a, cf_in)
                };
                let mut mem = GuestMemory::new();
                mem.map(0, 0x10, Perms::RX).unwrap();
                mem.load(0, &code).unwrap();
                let mut cpu = Cpu::new();
                cpu.regs.gpr[0] = a;
                cpu.regs.set_flag(flags::CF, cf_in);
                cpu.step(&mut mem).unwrap();
                assert_eq!(cpu.regs.gpr[0], hw.0, "{insn} {a:#x}");
                assert_eq!(cpu.regs.rflags & flags::STATUS, hw.1 & flags::STATUS, "{insn} {a:#x} cf_in={cf_in}");
            }
        }
    }
}
