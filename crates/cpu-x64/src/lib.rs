//! x86-64 CPU core (documented subset — see `decoder.rs` for the exact list).
//!
//! The PS5 CPU is an AMD Zen 2 x86-64 part. This crate models the user-mode
//! integer core only. NOT YET IMPLEMENTED: 8/16-bit operand sizes, ADC/SBB,
//! shifts, MUL/DIV, SSE/AVX, x87, system instructions, interrupts, a JIT.

pub mod decoder;
pub mod interpreter;
pub mod registers;

pub use decoder::{decode, DecodeError, Instruction};
pub use interpreter::{Cpu, CpuError, RunResult, StopReason};
pub use registers::{flags, Gpr, Registers, GPR_NAMES};
