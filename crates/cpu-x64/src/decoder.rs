//! x86-64 instruction decoder for the supported subset.
//!
//! Anything outside the subset returns [`DecodeError::Unsupported`] with the
//! raw bytes, so the caller can report exactly which instruction is missing.
//! Nothing is silently skipped.
//!
//! Supported (64-bit mode, 32- and 64-bit operand sizes):
//! - ALU:   ADD/OR/AND/SUB/XOR/CMP  (01 03 09 0B 21 23 29 2B 31 33 39 3B, 81 /0,1,4,5,6,7, 83 /0,1,4,5,6,7)
//! - MOV:   89, 8B, C7 /0, B8+r (imm32 / imm64 with REX.W)
//! - LEA:   8D
//! - INC/DEC: FF /0, FF /1
//! - Branches: 70-7F (Jcc rel8), 0F 80-8F (Jcc rel32), EB, E9, E8 (CALL rel32), C3 (RET)
//! - Stack: 50+r PUSH, 58+r POP
//! - NOP (90), HLT (F4)
//! - Addressing: [reg], [reg+disp8/32], SIB with scale/index/base, [disp32] via SIB, RIP-relative
//!
//! Prefixes other than REX (operand-size 66, LOCK, REP, segment overrides,
//! VEX/EVEX…) are reported as unsupported.

use std::fmt;

pub const MAX_INSN_LEN: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    W32,
    W64,
}

impl Width {
    pub fn bytes(self) -> usize {
        match self {
            Width::W32 => 4,
            Width::W64 => 8,
        }
    }
    pub fn mask(self) -> u64 {
        match self {
            Width::W32 => 0xFFFF_FFFF,
            Width::W64 => u64::MAX,
        }
    }
    pub fn sign_bit(self) -> u64 {
        match self {
            Width::W32 => 1 << 31,
            Width::W64 => 1 << 63,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemRef {
    pub base: Option<u8>,
    /// (register, scale) where scale is 1, 2, 4 or 8.
    pub index: Option<(u8, u8)>,
    pub disp: i64,
    /// Effective address = address of the next instruction + disp.
    pub rip_relative: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    Reg(u8),
    Mem(MemRef),
    /// Immediate, already sign/zero-extended to 64 bits as the encoding requires.
    Imm(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AluOp {
    Add,
    Or,
    And,
    Sub,
    Xor,
    Cmp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Alu { op: AluOp, width: Width, dst: Operand, src: Operand },
    Mov { width: Width, dst: Operand, src: Operand },
    Lea { width: Width, dst: u8, mem: MemRef },
    Inc { width: Width, dst: Operand },
    Dec { width: Width, dst: Operand },
    /// Condition code 0..=15 in x86 order (O, NO, B, AE, E, NE, BE, A, S, NS, P, NP, L, GE, LE, G).
    Jcc { cond: u8, rel: i64 },
    Jmp { rel: i64 },
    Call { rel: i64 },
    Ret,
    Push { reg: u8 },
    Pop { reg: u8 },
    Nop,
    Hlt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    pub op: Op,
    pub len: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// A valid x86 instruction this emulator does not implement yet.
    Unsupported { bytes: Vec<u8>, reason: &'static str },
    /// Ran out of bytes before the instruction was complete.
    Truncated,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Unsupported { bytes, reason } => {
                write!(f, "unsupported instruction (")?;
                for (i, b) in bytes.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{b:02X}")?;
                }
                write!(f, "): {reason}")
            }
            DecodeError::Truncated => write!(f, "instruction truncated"),
        }
    }
}

impl std::error::Error for DecodeError {}

#[derive(Clone, Copy, Default)]
struct Rex {
    w: bool,
    r: bool,
    x: bool,
    b: bool,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn u8(&mut self) -> Result<u8, DecodeError> {
        let b = *self.bytes.get(self.pos).ok_or(DecodeError::Truncated)?;
        self.pos += 1;
        if self.pos > MAX_INSN_LEN {
            return Err(DecodeError::Truncated);
        }
        Ok(b)
    }
    fn i8(&mut self) -> Result<i64, DecodeError> {
        Ok(self.u8()? as i8 as i64)
    }
    fn i32(&mut self) -> Result<i64, DecodeError> {
        let mut v = [0u8; 4];
        for b in &mut v {
            *b = self.u8()?;
        }
        Ok(i32::from_le_bytes(v) as i64)
    }
    fn u32(&mut self) -> Result<u64, DecodeError> {
        Ok(self.i32()? as u32 as u64)
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        let mut v = [0u8; 8];
        for b in &mut v {
            *b = self.u8()?;
        }
        Ok(u64::from_le_bytes(v))
    }
}

/// Decode ModRM (+SIB, +displacement). Returns (reg field incl. REX.R, r/m operand).
fn modrm(c: &mut Cursor, rex: Rex) -> Result<(u8, Operand), DecodeError> {
    let m = c.u8()?;
    let md = m >> 6;
    let reg = ((m >> 3) & 7) | if rex.r { 8 } else { 0 };
    let rm = m & 7;

    if md == 3 {
        return Ok((reg, Operand::Reg(rm | if rex.b { 8 } else { 0 })));
    }

    let mut mem = MemRef { base: None, index: None, disp: 0, rip_relative: false };

    if rm == 4 {
        // SIB byte follows.
        let sib = c.u8()?;
        let scale = 1u8 << (sib >> 6);
        let idx = ((sib >> 3) & 7) | if rex.x { 8 } else { 0 };
        let base = sib & 7;
        if idx != 4 {
            mem.index = Some((idx, scale));
        }
        if base == 5 && md == 0 {
            mem.disp = c.i32()?; // [index*scale + disp32], no base
        } else {
            mem.base = Some(base | if rex.b { 8 } else { 0 });
        }
    } else if rm == 5 && md == 0 {
        mem.rip_relative = true;
        mem.disp = c.i32()?;
    } else {
        mem.base = Some(rm | if rex.b { 8 } else { 0 });
    }

    match md {
        1 => mem.disp = c.i8()?,
        2 => mem.disp = c.i32()?,
        _ => {}
    }
    Ok((reg, Operand::Mem(mem)))
}

fn group1(ext: u8) -> Option<AluOp> {
    match ext {
        0 => Some(AluOp::Add),
        1 => Some(AluOp::Or),
        4 => Some(AluOp::And),
        5 => Some(AluOp::Sub),
        6 => Some(AluOp::Xor),
        7 => Some(AluOp::Cmp),
        _ => None, // 2 = ADC, 3 = SBB: not implemented yet
    }
}

/// Decode one instruction from the start of `bytes`.
pub fn decode(bytes: &[u8]) -> Result<Instruction, DecodeError> {
    let mut c = Cursor { bytes, pos: 0 };
    let unsupported = |c: &Cursor, reason| DecodeError::Unsupported {
        bytes: bytes[..c.pos.min(bytes.len())].to_vec(),
        reason,
    };

    let mut b = c.u8()?;
    match b {
        0x66 | 0x67 | 0xF0 | 0xF2 | 0xF3 | 0x26 | 0x2E | 0x36 | 0x3E | 0x64 | 0x65 => {
            return Err(unsupported(&c, "legacy prefix"));
        }
        0xC4 | 0xC5 | 0x62 => return Err(unsupported(&c, "VEX/EVEX encoding")),
        _ => {}
    }

    let mut rex = Rex::default();
    if (0x40..=0x4F).contains(&b) {
        rex = Rex { w: b & 8 != 0, r: b & 4 != 0, x: b & 2 != 0, b: b & 1 != 0 };
        b = c.u8()?;
    }
    let width = if rex.w { Width::W64 } else { Width::W32 };
    let rb = if rex.b { 8 } else { 0 };

    let op = match b {
        // ALU r/m, r  and  r, r/m
        0x01 | 0x03 | 0x09 | 0x0B | 0x21 | 0x23 | 0x29 | 0x2B | 0x31 | 0x33 | 0x39 | 0x3B => {
            let alu = match b & 0xF8 {
                0x00 => AluOp::Add,
                0x08 => AluOp::Or,
                0x20 => AluOp::And,
                0x28 => AluOp::Sub,
                0x30 => AluOp::Xor,
                _ => AluOp::Cmp,
            };
            let (reg, rm) = modrm(&mut c, rex)?;
            if b & 2 == 0 {
                Op::Alu { op: alu, width, dst: rm, src: Operand::Reg(reg) }
            } else {
                Op::Alu { op: alu, width, dst: Operand::Reg(reg), src: rm }
            }
        }
        0x81 | 0x83 => {
            let (ext, rm) = modrm(&mut c, rex)?;
            let alu = group1(ext & 7).ok_or_else(|| unsupported(&c, "ADC/SBB not implemented"))?;
            let imm = if b == 0x81 { c.i32()? } else { c.i8()? } as u64;
            Op::Alu { op: alu, width, dst: rm, src: Operand::Imm(imm) }
        }
        0x89 => {
            let (reg, rm) = modrm(&mut c, rex)?;
            Op::Mov { width, dst: rm, src: Operand::Reg(reg) }
        }
        0x8B => {
            let (reg, rm) = modrm(&mut c, rex)?;
            Op::Mov { width, dst: Operand::Reg(reg), src: rm }
        }
        0x8D => {
            let (reg, rm) = modrm(&mut c, rex)?;
            match rm {
                Operand::Mem(mem) => Op::Lea { width, dst: reg, mem },
                _ => return Err(unsupported(&c, "LEA with register operand is #UD")),
            }
        }
        0xC7 => {
            let (ext, rm) = modrm(&mut c, rex)?;
            if ext & 7 != 0 {
                return Err(unsupported(&c, "C7 with reg field != 0"));
            }
            let imm = c.i32()? as u64; // sign-extended; masked to 32 bits for W32
            Op::Mov { width, dst: rm, src: Operand::Imm(imm) }
        }
        0xB8..=0xBF => {
            let reg = (b - 0xB8) | rb;
            let imm = if rex.w { c.u64()? } else { c.u32()? };
            Op::Mov { width, dst: Operand::Reg(reg), src: Operand::Imm(imm) }
        }
        0xFF => {
            let (ext, rm) = modrm(&mut c, rex)?;
            match ext & 7 {
                0 => Op::Inc { width, dst: rm },
                1 => Op::Dec { width, dst: rm },
                _ => return Err(unsupported(&c, "FF group: only INC/DEC implemented")),
            }
        }
        0x70..=0x7F => Op::Jcc { cond: b & 0xF, rel: c.i8()? },
        0x0F => {
            let b2 = c.u8()?;
            match b2 {
                0x80..=0x8F => Op::Jcc { cond: b2 & 0xF, rel: c.i32()? },
                _ => return Err(unsupported(&c, "two-byte opcode not implemented")),
            }
        }
        0xEB => Op::Jmp { rel: c.i8()? },
        0xE9 => Op::Jmp { rel: c.i32()? },
        0xE8 => Op::Call { rel: c.i32()? },
        0xC3 => Op::Ret,
        0x50..=0x57 => Op::Push { reg: (b - 0x50) | rb },
        0x58..=0x5F => Op::Pop { reg: (b - 0x58) | rb },
        0x90 if !rex.b => Op::Nop,
        0xF4 => Op::Hlt,
        _ => return Err(unsupported(&c, "opcode not implemented")),
    };

    Ok(Instruction { op, len: c.pos as u8 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mov_rax_imm32_sign_extended() {
        // 48 C7 C0 07 00 00 00   mov rax, 7
        let i = decode(&[0x48, 0xC7, 0xC0, 7, 0, 0, 0]).unwrap();
        assert_eq!(i.len, 7);
        assert_eq!(i.op, Op::Mov { width: Width::W64, dst: Operand::Reg(0), src: Operand::Imm(7) });
    }

    #[test]
    fn movabs_r9() {
        // 49 B9 imm64   movabs r9, 0x1122334455667788
        let i = decode(&[0x49, 0xB9, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]).unwrap();
        assert_eq!(i.len, 10);
        assert_eq!(
            i.op,
            Op::Mov { width: Width::W64, dst: Operand::Reg(9), src: Operand::Imm(0x1122334455667788) }
        );
    }

    #[test]
    fn sib_with_index_and_disp8() {
        // 48 8B 44 8B 10   mov rax, [rbx + rcx*4 + 0x10]
        let i = decode(&[0x48, 0x8B, 0x44, 0x8B, 0x10]).unwrap();
        assert_eq!(i.len, 5);
        let Op::Mov { src: Operand::Mem(m), .. } = i.op else { panic!() };
        assert_eq!(m, MemRef { base: Some(3), index: Some((1, 4)), disp: 0x10, rip_relative: false });
    }

    #[test]
    fn rip_relative() {
        // 48 8B 05 F9 FF FF FF   mov rax, [rip - 7]
        let i = decode(&[0x48, 0x8B, 0x05, 0xF9, 0xFF, 0xFF, 0xFF]).unwrap();
        let Op::Mov { src: Operand::Mem(m), .. } = i.op else { panic!() };
        assert!(m.rip_relative);
        assert_eq!(m.disp, -7);
    }

    #[test]
    fn rbp_and_r13_base_need_displacement() {
        // 48 8B 45 00  mov rax, [rbp+0]
        let i = decode(&[0x48, 0x8B, 0x45, 0x00]).unwrap();
        let Op::Mov { src: Operand::Mem(m), .. } = i.op else { panic!() };
        assert_eq!(m.base, Some(5));
        // 49 8B 45 08  mov rax, [r13+8]
        let i = decode(&[0x49, 0x8B, 0x45, 0x08]).unwrap();
        let Op::Mov { src: Operand::Mem(m), .. } = i.op else { panic!() };
        assert_eq!((m.base, m.disp), (Some(13), 8));
    }

    #[test]
    fn unsupported_is_reported_with_bytes() {
        // 0F A2 = CPUID
        match decode(&[0x0F, 0xA2]) {
            Err(DecodeError::Unsupported { bytes, .. }) => assert_eq!(bytes, vec![0x0F, 0xA2]),
            other => panic!("{other:?}"),
        }
        assert!(matches!(decode(&[0x66, 0x90]), Err(DecodeError::Unsupported { .. })));
    }

    #[test]
    fn truncated() {
        assert_eq!(decode(&[0x48, 0xC7, 0xC0, 7]), Err(DecodeError::Truncated));
        assert_eq!(decode(&[]), Err(DecodeError::Truncated));
    }
}
