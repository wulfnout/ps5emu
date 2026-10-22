//! Reference guest programs.
//!
//! Each program's source is in `asm/*.s`; the `.bin` next to it is the raw
//! machine code produced by GNU `as` (see `asm/build.sh`). The emulator never
//! sees the assembly — only the bytes a real assembler emitted.
//!
//! Standard machine layout used by every program:
//!   0x0000–0x0FFF  code   (read + execute)
//!   0x1000–0x1FFF  data   (read + write)
//!   0x2000–0x2FFF  stack  (read + write), RSP starts at 0x3000

pub const CODE_BASE: u64 = 0x0000;
pub const CODE_SIZE: u64 = 0x1000;
pub const DATA_BASE: u64 = 0x1000;
pub const DATA_SIZE: u64 = 0x1000;
pub const STACK_BASE: u64 = 0x2000;
pub const STACK_SIZE: u64 = 0x1000;

// Register indices in x86-64 encoding order.
pub const RAX: u8 = 0;
pub const RCX: u8 = 1;
pub const RDX: u8 = 2;
pub const RBX: u8 = 3;
pub const RSP: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedStop {
    Halted,
    /// Execution must stop on an unimplemented instruction at this RIP.
    Unsupported { rip: u64 },
    /// Execution must stop on a memory fault at this RIP, accessing this address.
    MemoryFault { rip: u64, addr: u64 },
}

#[derive(Debug, Clone, Copy)]
pub struct TestProgram {
    pub name: &'static str,
    pub description: &'static str,
    pub source: &'static str,
    pub code: &'static [u8],
    pub expect_stop: ExpectedStop,
    /// (register index, value)
    pub expect_regs: &'static [(u8, u64)],
    /// (address, 64-bit value)
    pub expect_mem: &'static [(u64, u64)],
    /// Expected ZF after the run, if checked.
    pub expect_zf: Option<bool>,
}

pub const ALL: &[TestProgram] = &[
    TestProgram {
        name: "add_and_store",
        description: "Roadmap milestone: rax = 7, add 5, store to [0x1000], read back into rcx, compare with 12.",
        source: include_str!("../asm/add_and_store.s"),
        code: include_bytes!("../asm/add_and_store.bin"),
        expect_stop: ExpectedStop::Halted,
        expect_regs: &[(RAX, 12), (RCX, 12), (RBX, 0x1000)],
        expect_mem: &[(0x1000, 12)],
        expect_zf: Some(true),
    },
    TestProgram {
        name: "sum_loop",
        description: "Loop summing 1..10, then CALL a subroutine that uses RIP-relative data and RET; PUSH/POP round-trip.",
        source: include_str!("../asm/sum_loop.s"),
        code: include_bytes!("../asm/sum_loop.bin"),
        expect_stop: ExpectedStop::Halted,
        expect_regs: &[(RAX, 55), (RCX, 0), (RDX, 55), (RSP, 0x3000)],
        expect_mem: &[(0x1008, 55)],
        expect_zf: None,
    },
    TestProgram {
        name: "array_sum",
        description: "Sum a 4-element array including a negative value using [base + index*8] addressing and a signed JL loop.",
        source: include_str!("../asm/array_sum.s"),
        code: include_bytes!("../asm/array_sum.bin"),
        expect_stop: ExpectedStop::Halted,
        // 100 + (-30) + 7 + 0x7FFFFFFF = 2_147_483_724
        expect_regs: &[(RAX, 2_147_483_724), (RBX, 4)],
        expect_mem: &[(0x1008, (-30i64) as u64), (0x1020, 2_147_483_724)],
        expect_zf: Some(true),
    },
    TestProgram {
        name: "unsupported_cpuid",
        description: "Hits CPUID, which is not implemented. Must stop with an explicit unsupported-instruction error, not skip it.",
        source: include_str!("../asm/unsupported.s"),
        code: include_bytes!("../asm/unsupported.bin"),
        expect_stop: ExpectedStop::Unsupported { rip: 5 },
        expect_regs: &[(RAX, 1)],
        expect_mem: &[],
        expect_zf: None,
    },
    TestProgram {
        name: "memory_fault",
        description: "Stores to unmapped address 0xDEAD0000. Must raise a precise fault at the store; RIP stays on the faulting instruction.",
        source: include_str!("../asm/memory_fault.s"),
        code: include_bytes!("../asm/memory_fault.bin"),
        expect_stop: ExpectedStop::MemoryFault { rip: 0x11, addr: 0xDEAD_0000 },
        expect_regs: &[(RAX, 42), (RBX, 0xDEAD_0000)],
        expect_mem: &[],
        expect_zf: None,
    },
];
