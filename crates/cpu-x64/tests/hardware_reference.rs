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

// ---------------------------------------------------------------------------
// TEST, NEG, NOT, shifts and IMUL against the host CPU.

use cpu_x64::decoder::ShiftKind;
use cpu_x64::interpreter::{imul, neg, shift};

/// Run `insn` with a and b in registers, returning (dst, rflags).
/// Flags are first set to a known pattern so "unchanged" cases can be detected.
macro_rules! hw2 {
    ($insn:literal, $suffix:literal, $a:expr, $b:expr, $preset:expr) => {{
        let mut r: u64 = $a;
        let fl: u64;
        unsafe {
            asm!("push {p}", "popfq", concat!($insn, " {r", $suffix, "}, {b", $suffix, "}"), "pushfq", "pop {f}",
                 r = inout(reg) r, b = in(reg) $b, p = in(reg) $preset, f = out(reg) fl);
        }
        (r, fl)
    }};
}

/// Shifts by CL with count n.
macro_rules! hw_shift {
    ($insn:literal, $suffix:literal, $a:expr, $n:expr, $preset:expr) => {{
        let mut r: u64 = $a;
        let fl: u64;
        unsafe {
            asm!("push {p}", "popfq", concat!($insn, " {r", $suffix, "}, cl"), "pushfq", "pop {f}",
                 r = inout(reg) r, in("rcx") $n as u64, p = in(reg) $preset, f = out(reg) fl);
        }
        (r, fl)
    }};
}

macro_rules! hw1 {
    ($insn:literal, $suffix:literal, $a:expr, $preset:expr) => {{
        let mut r: u64 = $a;
        let fl: u64;
        unsafe {
            asm!("push {p}", "popfq", concat!($insn, " {r", $suffix, "}"), "pushfq", "pop {f}",
                 r = inout(reg) r, p = in(reg) $preset, f = out(reg) fl);
        }
        (r, fl)
    }};
}

const PRESETS: [u64; 2] = [0x2, 0x2 | flags::STATUS];

fn w_mask(w: Width) -> u64 {
    w.mask()
}

#[test]
fn test_neg_not_match_hardware() {
    let ops = operands();
    for &p in &PRESETS {
        for &a in &ops {
            for &b in &ops {
                let hw = hw2!("test", "", a, b, p);
                check(AluOp::Test, Width::W64, a, b, (a & b, hw.1));
                let hw = hw2!("test", ":e", a, b, p);
                check(AluOp::Test, Width::W32, a, b, ((a & b) & 0xFFFF_FFFF, hw.1));
            }
            for (w, hw) in [(Width::W64, hw1!("neg", "", a, p)), (Width::W32, hw1!("neg", ":e", a, p))] {
                let (r, f) = neg(w, a);
                assert_eq!(r, hw.0 & w_mask(w), "neg {w:?} {a:#x}");
                assert_eq!(f & flags::STATUS, hw.1 & flags::STATUS, "neg flags {w:?} {a:#x}");
            }
            // NOT changes no flags.
            let hw = hw1!("not", "", a, p);
            assert_eq!(hw.1 & flags::STATUS, p & flags::STATUS);
            assert_eq!(hw.0, !a);
        }
    }
}

#[test]
fn shifts_match_hardware() {
    let ops = operands();
    let mut n_cmp = 0;
    for &p in &PRESETS {
        for &a in &ops {
            for n in [0u8, 1, 2, 3, 7, 8, 15, 16, 31, 32, 33, 63, 64, 65, 255] {
                for (kind, name) in [(ShiftKind::Shl, "shl"), (ShiftKind::Shr, "shr"), (ShiftKind::Sar, "sar")] {
                    for w in [Width::W64, Width::W32] {
                        let hw = match (name, w) {
                            ("shl", Width::W64) => hw_shift!("shl", "", a, n, p),
                            ("shr", Width::W64) => hw_shift!("shr", "", a, n, p),
                            ("sar", Width::W64) => hw_shift!("sar", "", a, n, p),
                            ("shl", _) => hw_shift!("shl", ":e", a, n, p),
                            ("shr", _) => hw_shift!("shr", ":e", a, n, p),
                            _ => hw_shift!("sar", ":e", a, n, p),
                        };
                        let hw_r = hw.0 & w_mask(w);
                        let masked = n & if w == Width::W64 { 0x3F } else { 0x1F };
                        match shift(kind, w, a, n) {
                            None => {
                                assert_eq!(masked, 0);
                                // count 0: value (low bits) and flags unchanged
                                assert_eq!(hw_r, a & w_mask(w), "{name} {w:?} {a:#x} by {n}");
                                assert_eq!(hw.1 & flags::STATUS, p & flags::STATUS, "{name} {w:?} by 0 flags");
                            }
                            Some((r, f)) => {
                                assert_eq!(r, hw_r, "{name} {w:?} {a:#x} by {n}");
                                // AF is undefined; OF only defined for count 1.
                                let mut m = flags::CF | flags::ZF | flags::SF | flags::PF;
                                if masked == 1 {
                                    m |= flags::OF;
                                }
                                assert_eq!(f & m, hw.1 & m, "{name} {w:?} {a:#x} by {n}: ours {f:#x} hw {:#x}", hw.1);
                            }
                        }
                        n_cmp += 1;
                    }
                }
            }
        }
    }
    eprintln!("compared {n_cmp} shifts against the host CPU");
}

#[test]
fn imul_matches_hardware() {
    let ops = operands();
    for &a in &ops {
        for &b in &ops {
            let hw = hw2!("imul", "", a, b, 0x2u64);
            let (r, f) = imul(Width::W64, a, b);
            let m = flags::CF | flags::OF; // SF/ZF/AF/PF undefined
            assert_eq!(r, hw.0, "imul64 {a:#x} {b:#x}");
            assert_eq!(f & m, hw.1 & m, "imul64 flags {a:#x} {b:#x}");

            let hw = hw2!("imul", ":e", a, b, 0x2u64);
            let (r, f) = imul(Width::W32, a, b);
            assert_eq!(r, hw.0 & 0xFFFF_FFFF, "imul32 {a:#x} {b:#x}");
            assert_eq!(f & m, hw.1 & m, "imul32 flags {a:#x} {b:#x}");
        }
    }
}

/// CMOV through the full interpreter, including the 64-bit-mode quirk that a
/// 32-bit CMOV clears the upper half of the destination even when not taken.
#[test]
fn cmov_32bit_zero_extends_even_when_not_taken() {
    use cpu_x64::Cpu;
    use guest_memory::{GuestMemory, Perms};
    let a: u64 = 0xFFFF_FFFF_1234_5678;
    let b: u64 = 0xAAAA_AAAA_BBBB_BBBB;
    for (zf, code) in [(false, [0x0Fu8, 0x44, 0xC3, 0xF4]), (true, [0x0F, 0x44, 0xC3, 0xF4])] {
        // 0F 44 C3 = cmove eax, ebx
        let hw: u64;
        unsafe {
            let p: u64 = if zf { 0x2 | flags::ZF } else { 0x2 };
            let mut r = a;
            asm!("push {p}", "popfq", "cmove {r:e}, {b:e}", r = inout(reg) r, b = in(reg) b, p = in(reg) p);
            hw = r;
        }
        let mut mem = GuestMemory::new();
        mem.map(0, 0x10, Perms::RX).unwrap();
        mem.load(0, &code).unwrap();
        let mut cpu = Cpu::new();
        cpu.regs.gpr[0] = a;
        cpu.regs.gpr[3] = b;
        cpu.regs.set_flag(flags::ZF, zf);
        cpu.step(&mut mem).unwrap();
        assert_eq!(cpu.regs.gpr[0], hw, "cmove taken={zf}");
    }
}
