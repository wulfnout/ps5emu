//! Instruction interpreter.
//!
//! Exceptions are precise: if an instruction faults, RIP still points at it
//! and no register has been modified.

use crate::decoder::{decode, AluOp, DecodeError, MemRef, Op, Operand, ShiftCount, ShiftKind, Width, MAX_INSN_LEN};
use crate::registers::{flags, Gpr, Registers};
use guest_memory::{Access, GuestMemory, MemoryFault};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CpuError {
    Decode { rip: u64, error: DecodeError },
    Memory { rip: u64, fault: MemoryFault },
    /// The CPU executed HLT and is stopped.
    Halted,
}

impl fmt::Display for CpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpuError::Decode { rip, error } => write!(f, "at rip={rip:#x}: {error}"),
            CpuError::Memory { rip, fault } => write!(f, "at rip={rip:#x}: {fault}"),
            CpuError::Halted => write!(f, "cpu halted"),
        }
    }
}

impl std::error::Error for CpuError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    Halted,
    StepLimit,
    Error(CpuError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    pub steps: u64,
    pub stop: StopReason,
}

#[derive(Debug, Clone, Default)]
pub struct Cpu {
    pub regs: Registers,
    pub halted: bool,
    pub instructions_retired: u64,
}

fn parity_even(v: u64) -> bool {
    (v as u8).count_ones().is_multiple_of(2)
}

/// Pure ALU result + status flags, shared by the interpreter and tests.
/// Returns (result, new status flag bits). For INC/DEC the caller preserves CF.
pub fn alu(op: AluOp, width: Width, a: u64, b: u64) -> (u64, u64) {
    let mask = width.mask();
    let sign = width.sign_bit();
    let (a, b) = (a & mask, b & mask);
    let mut f = 0u64;
    let r = match op {
        AluOp::Add => {
            let r = a.wrapping_add(b) & mask;
            if (a as u128 + b as u128) > mask as u128 {
                f |= flags::CF;
            }
            if (a ^ r) & (b ^ r) & sign != 0 {
                f |= flags::OF;
            }
            if (a ^ b ^ r) & 0x10 != 0 {
                f |= flags::AF;
            }
            r
        }
        AluOp::Sub | AluOp::Cmp => {
            let r = a.wrapping_sub(b) & mask;
            if a < b {
                f |= flags::CF;
            }
            if (a ^ b) & (a ^ r) & sign != 0 {
                f |= flags::OF;
            }
            if (a ^ b ^ r) & 0x10 != 0 {
                f |= flags::AF;
            }
            r
        }
        // Logic ops clear CF and OF. AF is architecturally undefined; we clear it.
        AluOp::And | AluOp::Test => a & b,
        AluOp::Or => a | b,
        AluOp::Xor => a ^ b,
    };
    if r == 0 {
        f |= flags::ZF;
    }
    if r & sign != 0 {
        f |= flags::SF;
    }
    if parity_even(r) {
        f |= flags::PF;
    }
    (r, f)
}

fn result_flags(r: u64, width: Width) -> u64 {
    let mut f = 0;
    if r == 0 {
        f |= flags::ZF;
    }
    if r & width.sign_bit() != 0 {
        f |= flags::SF;
    }
    if parity_even(r) {
        f |= flags::PF;
    }
    f
}

/// Shift result and flags. Returns None when the masked count is 0: the
/// destination and all flags are left unchanged, as on hardware.
/// OF is only architecturally defined for a count of 1; for larger counts we
/// compute it the same way, and tests compare it only for count 1.
pub fn shift(kind: ShiftKind, width: Width, a: u64, count: u8) -> Option<(u64, u64)> {
    let bits = (width.bytes() * 8) as u32;
    let n = (count as u32) & if width == Width::W64 { 0x3F } else { 0x1F };
    if n == 0 {
        return None;
    }
    let mask = width.mask();
    let a = a & mask;
    let sign = width.sign_bit();
    let (r, cf) = match kind {
        ShiftKind::Shl => ((a << n) & mask, (a >> (bits - n)) & 1 != 0),
        ShiftKind::Shr => (a >> n, (a >> (n - 1)) & 1 != 0),
        ShiftKind::Sar => {
            let sa = if width == Width::W64 { a as i64 } else { a as u32 as i32 as i64 };
            ((sa >> n) as u64 & mask, (sa >> (n - 1)) & 1 != 0)
        }
    };
    let of = match kind {
        ShiftKind::Shl => (r & sign != 0) != cf,
        ShiftKind::Shr => a & sign != 0,
        ShiftKind::Sar => false,
    };
    let mut f = result_flags(r, width);
    if cf {
        f |= flags::CF;
    }
    if of {
        f |= flags::OF;
    }
    Some((r, f))
}

/// Truncating signed multiply. CF = OF = 1 when the full signed product does
/// not fit in the destination width. SF/ZF/PF are set from the result (they
/// are architecturally undefined; tests compare only CF and OF).
pub fn imul(width: Width, a: u64, b: u64) -> (u64, u64) {
    let (sa, sb) = match width {
        Width::W64 => (a as i64 as i128, b as i64 as i128),
        Width::W32 => (a as u32 as i32 as i128, b as u32 as i32 as i128),
    };
    let full = sa * sb;
    let r = (full as u64) & width.mask();
    let fits = match width {
        Width::W64 => full == (full as i64) as i128,
        Width::W32 => full == (full as i32) as i128,
    };
    let mut f = result_flags(r, width);
    if !fits {
        f |= flags::CF | flags::OF;
    }
    (r, f)
}

/// NEG: 0 - a, with CF set when a != 0.
pub fn neg(width: Width, a: u64) -> (u64, u64) {
    let (r, mut f) = alu(AluOp::Sub, width, 0, a);
    f &= !flags::CF;
    if a & width.mask() != 0 {
        f |= flags::CF;
    }
    (r, f)
}

impl Cpu {
    pub fn new() -> Self {
        Self::default()
    }

    fn ea(&self, m: &MemRef, next_rip: u64) -> u64 {
        let mut a = m.disp as u64;
        if m.rip_relative {
            a = a.wrapping_add(next_rip);
        }
        if let Some(b) = m.base {
            a = a.wrapping_add(self.regs.gpr[b as usize]);
        }
        if let Some((i, s)) = m.index {
            a = a.wrapping_add(self.regs.gpr[i as usize].wrapping_mul(s as u64));
        }
        a
    }

    fn read_op(&self, mem: &GuestMemory, o: &Operand, w: Width, next: u64) -> Result<u64, MemoryFault> {
        Ok(match o {
            Operand::Reg(r) => self.regs.gpr[*r as usize] & w.mask(),
            Operand::Imm(v) => *v & w.mask(),
            Operand::Mem(m) => {
                let addr = self.ea(m, next);
                let mut buf = [0u8; 8];
                mem.read(addr, &mut buf[..w.bytes()], Access::Read)?;
                u64::from_le_bytes(buf)
            }
        })
    }

    /// 32-bit register writes zero-extend into the full 64-bit register.
    fn write_op(&mut self, mem: &mut GuestMemory, o: &Operand, w: Width, v: u64, next: u64) -> Result<(), MemoryFault> {
        match o {
            Operand::Reg(r) => self.regs.gpr[*r as usize] = v & w.mask(),
            Operand::Mem(m) => {
                let addr = self.ea(m, next);
                mem.write(addr, &v.to_le_bytes()[..w.bytes()])?;
            }
            Operand::Imm(_) => unreachable!("decoder never produces an immediate destination"),
        }
        Ok(())
    }

    fn set_status(&mut self, new: u64, keep_cf: bool) {
        let mut mask = flags::STATUS;
        if keep_cf {
            mask &= !flags::CF;
        }
        self.regs.rflags = (self.regs.rflags & !mask) | (new & mask);
    }

    pub fn condition(&self, cond: u8) -> bool {
        let r = &self.regs;
        let (cf, zf, sf, of, pf) = (
            r.flag(flags::CF),
            r.flag(flags::ZF),
            r.flag(flags::SF),
            r.flag(flags::OF),
            r.flag(flags::PF),
        );
        let base = match cond >> 1 {
            0 => of,
            1 => cf,
            2 => zf,
            3 => cf || zf,
            4 => sf,
            5 => pf,
            6 => sf != of,
            _ => zf || (sf != of),
        };
        if cond & 1 == 1 { !base } else { base }
    }

    /// Execute exactly one instruction.
    pub fn step(&mut self, mem: &mut GuestMemory) -> Result<(), CpuError> {
        if self.halted {
            return Err(CpuError::Halted);
        }
        let rip = self.regs.rip;
        let memerr = |fault| CpuError::Memory { rip, fault };

        let mut buf = [0u8; MAX_INSN_LEN];
        let n = mem.fetch(rip, &mut buf).map_err(memerr)?;
        let insn = decode(&buf[..n]).map_err(|error| CpuError::Decode { rip, error })?;
        let next = rip.wrapping_add(insn.len as u64);
        let mut new_rip = next;

        match insn.op {
            Op::Alu { op, width, dst, src } => {
                let a = self.read_op(mem, &dst, width, next).map_err(memerr)?;
                let b = self.read_op(mem, &src, width, next).map_err(memerr)?;
                let (r, f) = alu(op, width, a, b);
                if op != AluOp::Cmp && op != AluOp::Test {
                    self.write_op(mem, &dst, width, r, next).map_err(memerr)?;
                }
                self.set_status(f, false);
            }
            Op::Mov { width, dst, src } => {
                let v = self.read_op(mem, &src, width, next).map_err(memerr)?;
                self.write_op(mem, &dst, width, v, next).map_err(memerr)?;
            }
            Op::Lea { width, dst, mem: m } => {
                let a = self.ea(&m, next);
                self.regs.gpr[dst as usize] = a & width.mask();
            }
            Op::Inc { width, dst } | Op::Dec { width, dst } => {
                let alu_op = if matches!(insn.op, Op::Inc { .. }) { AluOp::Add } else { AluOp::Sub };
                let a = self.read_op(mem, &dst, width, next).map_err(memerr)?;
                let (r, f) = alu(alu_op, width, a, 1);
                self.write_op(mem, &dst, width, r, next).map_err(memerr)?;
                self.set_status(f, true);
            }
            Op::Not { width, dst } => {
                let a = self.read_op(mem, &dst, width, next).map_err(memerr)?;
                self.write_op(mem, &dst, width, !a, next).map_err(memerr)?;
            }
            Op::Neg { width, dst } => {
                let a = self.read_op(mem, &dst, width, next).map_err(memerr)?;
                let (r, f) = neg(width, a);
                self.write_op(mem, &dst, width, r, next).map_err(memerr)?;
                self.set_status(f, false);
            }
            Op::Shift { kind, width, dst, count } => {
                let n = match count {
                    ShiftCount::Imm(n) => n,
                    ShiftCount::Cl => self.regs.gpr[1] as u8,
                };
                let a = self.read_op(mem, &dst, width, next).map_err(memerr)?;
                if let Some((r, f)) = shift(kind, width, a, n) {
                    self.write_op(mem, &dst, width, r, next).map_err(memerr)?;
                    self.set_status(f, false);
                }
            }
            Op::Imul { width, dst, src, imm } => {
                let b = self.read_op(mem, &src, width, next).map_err(memerr)?;
                let a = match imm {
                    Some(_) => b,
                    None => self.regs.gpr[dst as usize] & width.mask(),
                };
                let m = match imm {
                    Some(i) => i,
                    None => b,
                };
                let (r, f) = imul(width, a, m);
                self.regs.gpr[dst as usize] = r;
                self.set_status(f, false);
            }
            Op::Cmov { cond, width, dst, src } => {
                // The source is read (and can fault) whether or not the move happens.
                let v = self.read_op(mem, &src, width, next).map_err(memerr)?;
                let d = dst as usize;
                self.regs.gpr[d] = if self.condition(cond) { v } else { self.regs.gpr[d] & width.mask() };
            }
            Op::Jcc { cond, rel } => {
                if self.condition(cond) {
                    new_rip = next.wrapping_add(rel as u64);
                }
            }
            Op::Jmp { rel } => new_rip = next.wrapping_add(rel as u64),
            Op::Call { rel } => {
                let sp = self.regs.get(Gpr::Rsp).wrapping_sub(8);
                mem.write_u64(sp, next).map_err(memerr)?;
                self.regs.set(Gpr::Rsp, sp);
                new_rip = next.wrapping_add(rel as u64);
            }
            Op::Ret => {
                let sp = self.regs.get(Gpr::Rsp);
                new_rip = mem.read_u64(sp).map_err(memerr)?;
                self.regs.set(Gpr::Rsp, sp.wrapping_add(8));
            }
            Op::Push { reg } => {
                let v = self.regs.gpr[reg as usize];
                let sp = self.regs.get(Gpr::Rsp).wrapping_sub(8);
                mem.write_u64(sp, v).map_err(memerr)?;
                self.regs.set(Gpr::Rsp, sp);
            }
            Op::Pop { reg } => {
                let sp = self.regs.get(Gpr::Rsp);
                let v = mem.read_u64(sp).map_err(memerr)?;
                self.regs.set(Gpr::Rsp, sp.wrapping_add(8));
                self.regs.gpr[reg as usize] = v; // pop rsp: loaded value wins, as on hardware
            }
            Op::Nop => {}
            Op::Hlt => self.halted = true,
        }

        self.regs.rip = new_rip;
        self.instructions_retired += 1;
        Ok(())
    }

    /// Run until HLT, an error, or `max_steps` instructions.
    pub fn run(&mut self, mem: &mut GuestMemory, max_steps: u64) -> RunResult {
        let mut steps = 0;
        while steps < max_steps {
            match self.step(mem) {
                Ok(()) => steps += 1,
                Err(e) => return RunResult { steps, stop: StopReason::Error(e) },
            }
            if self.halted {
                return RunResult { steps, stop: StopReason::Halted };
            }
        }
        RunResult { steps, stop: StopReason::StepLimit }
    }
}
