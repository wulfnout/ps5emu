//! Architectural register state.

/// General-purpose register numbers in x86-64 encoding order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Gpr {
    Rax = 0, Rcx = 1, Rdx = 2, Rbx = 3, Rsp = 4, Rbp = 5, Rsi = 6, Rdi = 7,
    R8 = 8, R9 = 9, R10 = 10, R11 = 11, R12 = 12, R13 = 13, R14 = 14, R15 = 15,
}

pub const GPR_NAMES: [&str; 16] = [
    "rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi",
    "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15",
];

/// RFLAGS bit positions.
pub mod flags {
    pub const CF: u64 = 1 << 0;
    pub const PF: u64 = 1 << 2;
    pub const AF: u64 = 1 << 4;
    pub const ZF: u64 = 1 << 6;
    pub const SF: u64 = 1 << 7;
    pub const OF: u64 = 1 << 11;
    /// The six arithmetic status flags.
    pub const STATUS: u64 = CF | PF | AF | ZF | SF | OF;
    /// Bit 1 of RFLAGS always reads as 1.
    pub const RESERVED_1: u64 = 1 << 1;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registers {
    pub gpr: [u64; 16],
    pub rip: u64,
    pub rflags: u64,
}

impl Default for Registers {
    fn default() -> Self {
        Self { gpr: [0; 16], rip: 0, rflags: flags::RESERVED_1 }
    }
}

impl Registers {
    pub fn get(&self, r: Gpr) -> u64 {
        self.gpr[r as usize]
    }

    pub fn set(&mut self, r: Gpr, v: u64) {
        self.gpr[r as usize] = v;
    }

    pub fn flag(&self, mask: u64) -> bool {
        self.rflags & mask != 0
    }

    pub fn set_flag(&mut self, mask: u64, on: bool) {
        if on {
            self.rflags |= mask;
        } else {
            self.rflags &= !mask;
        }
    }
}
