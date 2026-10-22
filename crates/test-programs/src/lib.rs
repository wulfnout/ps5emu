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
//!   0x100000–      screen (read + write), 256x144 x 32-bit pixels
//!   0x200000–      controller port (read only), see INPUT_BASE

pub const CODE_BASE: u64 = 0x0000;
pub const CODE_SIZE: u64 = 0x1000;
pub const DATA_BASE: u64 = 0x1000;
pub const DATA_SIZE: u64 = 0x1000;
pub const STACK_BASE: u64 = 0x2000;
pub const STACK_SIZE: u64 = 0x1000;

/// Memory-mapped screen: 256x144 pixels, 4 bytes each, 0x00RRGGBB little-endian.
pub const FB_BASE: u64 = 0x10_0000;
pub const FB_WIDTH: u32 = 256;
pub const FB_HEIGHT: u32 = 144;
pub const FB_SIZE: u64 = (FB_WIDTH * FB_HEIGHT * 4) as u64;

/// Controller port (guest read-only), all fields little-endian u32:
///   +0x00 frame counter (host increments once per frame)
///   +0x04 source: 0 none, 1 controller, 2 keyboard/mouse
///   +0x08 buttons bitmask (bit order = INPUT_BUTTONS)
///   +0x0C left stick X, +0x10 left stick Y, +0x14 right stick X, +0x18 right stick Y
///         (0..255, 128 = centered, like a DualSense report)
///   +0x1C L2 analog, +0x20 R2 analog (0..255)
pub const INPUT_BASE: u64 = 0x20_0000;
pub const INPUT_SIZE: u64 = 0x1000;

/// Button bit order. Matches the DualSense layout and the browser
/// "standard" gamepad mapping, so button N in the browser is bit N here.
pub const INPUT_BUTTONS: [&str; 18] = [
    "Cross", "Circle", "Square", "Triangle", "L1", "R1", "L2", "R2", "Create", "Options",
    "L3", "R3", "Up", "Down", "Left", "Right", "PS", "Touchpad",
];

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
    /// Program runs forever (e.g. waits for input frames); must still be running at the step limit.
    StepLimit,
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
    /// (address, 32-bit value) — used for screen pixels.
    pub expect_mem32: &'static [(u64, u32)],
    /// The program draws to the screen.
    pub draws: bool,
}

pub const ALL: &[TestProgram] = &[
    TestProgram {
        name: "controller_test",
        description: "Reads the controller port every frame: sticks move two squares, every button lights its slot in the bar. Runs forever, waiting for input frames.",
        source: include_str!("../asm/controller_test.s"),
        code: include_bytes!("../asm/controller_test.bin"),
        // With no input frames the program draws once, then waits forever.
        expect_stop: ExpectedStop::StepLimit,
        expect_regs: &[],
        expect_mem: &[(0x1000, 0)],
        expect_zf: Some(true),
        expect_mem32: &[
            (FB_BASE, 0xFFFFFF),           // left square at (0,0), sticks read 0
            (FB_BASE + 0x5050, 0x000000),  // empty background at (20,20)
            (FB_BASE + 0x22010, 0x232A3A), // Cross light at (4,136), off
        ],
        draws: true,
    },
    TestProgram {
        name: "draw_gradient",
        description: "Fills the 256x144 screen with a color gradient: red follows x, green follows y (IMUL + SHR), blue is x XOR y.",
        source: include_str!("../asm/draw_gradient.s"),
        code: include_bytes!("../asm/draw_gradient.bin"),
        expect_stop: ExpectedStop::Halted,
        expect_regs: &[(RCX, 144)],
        expect_mem: &[],
        expect_zf: Some(true),
        expect_mem32: &[(FB_BASE, 0x000000), (FB_BASE + 0x5028, 0x0A231E), (FB_BASE + 0x23FFC, 0xFFFA70)],
        draws: true,
    },
    TestProgram {
        name: "draw_checker",
        description: "Draws a checkerboard with a red border using TEST, CMOVcc and SAR.",
        source: include_str!("../asm/draw_checker.s"),
        code: include_bytes!("../asm/draw_checker.bin"),
        expect_stop: ExpectedStop::Halted,
        expect_regs: &[(RCX, 144)],
        expect_mem: &[],
        expect_zf: Some(true),
        expect_mem32: &[
            (FB_BASE, 0xFF5D6C),
            (FB_BASE + 0x5050, 0x1B2A49),
            (FB_BASE + 0x50A0, 0xF2F2F2),
            (FB_BASE + 0x23FFC, 0xF2F2F2),
        ],
        draws: true,
    },
    TestProgram {
        name: "add_and_store",
        description: "Roadmap milestone: rax = 7, add 5, store to [0x1000], read back into rcx, compare with 12.",
        source: include_str!("../asm/add_and_store.s"),
        code: include_bytes!("../asm/add_and_store.bin"),
        expect_stop: ExpectedStop::Halted,
        expect_regs: &[(RAX, 12), (RCX, 12), (RBX, 0x1000)],
        expect_mem: &[(0x1000, 12)],
        expect_zf: Some(true),
        expect_mem32: &[],
        draws: false,
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
        expect_mem32: &[],
        draws: false,
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
        expect_mem32: &[],
        draws: false,
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
        expect_mem32: &[],
        draws: false,
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
        expect_mem32: &[],
        draws: false,
    },
];
