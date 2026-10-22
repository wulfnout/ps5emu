//! C ABI over `emulator-core`.
//!
//! Deliberately dependency-free (no wasm-bindgen): the same exported functions
//! are used by
//!   - the browser, via `cargo build --target wasm32-unknown-unknown` and a
//!     ~60-line JS loader (apps/browser/src/core.ts), and
//!   - iOS, via `staticlib` + `include/emulator.h` bridged into Swift.
//!
//! Conventions
//! - Strings/JSON are returned through a per-thread result buffer: the call
//!   returns the byte length, and `emu_result_ptr()` gives the address. The
//!   buffer is valid until the next call that writes a result.
//! - Byte input (guest code) is passed as (ptr, len). In WebAssembly, the host
//!   gets a buffer from `emu_alloc` and frees it with `emu_free`.
//! - Machines are opaque handles; free them with `emu_machine_free`.

use emulator_core::cpu_x64::{CpuError, GPR_NAMES};
use emulator_core::{Machine, DEFAULT_STEP_LIMIT};
use std::cell::RefCell;

thread_local! {
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn set_result(s: String) -> usize {
    RESULT.with(|r| {
        let mut r = r.borrow_mut();
        *r = s.into_bytes();
        r.len()
    })
}

/// Address of the most recent result string (UTF-8, not NUL-terminated).
#[no_mangle]
pub extern "C" fn emu_result_ptr() -> *const u8 {
    RESULT.with(|r| r.borrow().as_ptr())
}

/// Allocate `len` bytes the host can write into (used by WebAssembly hosts).
#[no_mangle]
pub extern "C" fn emu_alloc(len: usize) -> *mut u8 {
    let mut v = vec![0u8; len.max(1)];
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Free a buffer from `emu_alloc`.
///
/// # Safety
/// `ptr`/`len` must come from exactly one prior `emu_alloc(len)` call.
#[no_mangle]
pub unsafe extern "C" fn emu_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        drop(Vec::from_raw_parts(ptr, len.max(1), len.max(1)));
    }
}

#[no_mangle]
pub extern "C" fn emu_core_version() -> usize {
    set_result(env!("CARGO_PKG_VERSION").to_string())
}

/// Run every reference program. Result: JSON array of reports.
/// Returns the JSON length.
#[no_mangle]
pub extern "C" fn emu_run_all_tests() -> usize {
    set_result(emulator_core::run_all_json())
}

/// Number of built-in reference programs.
#[no_mangle]
pub extern "C" fn emu_test_program_count() -> usize {
    emulator_core::test_programs::ALL.len()
}

pub struct MachineHandle {
    machine: Machine,
    last_error: Option<String>,
}

/// Create a machine (standard layout) and load raw x86-64 code at address 0.
/// Returns null if the code does not fit.
///
/// # Safety
/// `code` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn emu_machine_new(code: *const u8, len: usize) -> *mut MachineHandle {
    let bytes = if len == 0 { &[][..] } else { std::slice::from_raw_parts(code, len) };
    let mut machine = Machine::with_standard_layout();
    if machine.load_code(bytes).is_err() {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(MachineHandle { machine, last_error: None }))
}

/// Create a machine loaded with built-in reference program `index`.
#[no_mangle]
pub extern "C" fn emu_machine_new_test_program(index: usize) -> *mut MachineHandle {
    match emulator_core::test_programs::ALL.get(index) {
        Some(p) => unsafe { emu_machine_new(p.code.as_ptr(), p.code.len()) },
        None => std::ptr::null_mut(),
    }
}

/// # Safety
/// `h` must come from `emu_machine_new*` and not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn emu_machine_free(h: *mut MachineHandle) {
    if !h.is_null() {
        drop(Box::from_raw(h));
    }
}

pub const EMU_RUNNING: i32 = 0;
pub const EMU_HALTED: i32 = 1;
pub const EMU_ERROR: i32 = 2;

fn status(h: &MachineHandle) -> i32 {
    if h.last_error.is_some() {
        EMU_ERROR
    } else if h.machine.cpu.halted {
        EMU_HALTED
    } else {
        EMU_RUNNING
    }
}

/// Execute one instruction. Returns EMU_RUNNING / EMU_HALTED / EMU_ERROR.
///
/// # Safety
/// `h` must be a live handle.
#[no_mangle]
pub unsafe extern "C" fn emu_machine_step(h: *mut MachineHandle) -> i32 {
    let h = &mut *h;
    if status(h) != EMU_RUNNING {
        return status(h);
    }
    if let Err(e) = h.machine.cpu.step(&mut h.machine.memory) {
        h.last_error = Some(e.to_string());
    }
    status(h)
}

/// Run until halt/error or the step limit. Returns the status.
///
/// # Safety
/// `h` must be a live handle.
#[no_mangle]
pub unsafe extern "C" fn emu_machine_run(h: *mut MachineHandle) -> i32 {
    let h = &mut *h;
    if status(h) != EMU_RUNNING {
        return status(h);
    }
    let r = h.machine.cpu.run(&mut h.machine.memory, DEFAULT_STEP_LIMIT);
    if let emulator_core::cpu_x64::StopReason::Error(e) = r.stop {
        if e != CpuError::Halted {
            h.last_error = Some(e.to_string());
        }
    }
    status(h)
}

/// Machine state as JSON: registers, rip, rflags, status, error, data preview.
/// Returns the JSON length.
///
/// # Safety
/// `h` must be a live handle.
#[no_mangle]
pub unsafe extern "C" fn emu_machine_state_json(h: *const MachineHandle) -> usize {
    let h = &*h;
    let cpu = &h.machine.cpu;
    let regs: Vec<String> = GPR_NAMES
        .iter()
        .zip(cpu.regs.gpr)
        .map(|(n, v)| format!(r#"["{n}","{v:#018x}"]"#))
        .collect();
    let data: Vec<String> = (0..8u64)
        .map(|i| {
            let a = emulator_core::test_programs::DATA_BASE + i * 8;
            format!(r#"["{a:#06x}","{:#018x}"]"#, h.machine.memory.read_u64(a).unwrap_or(0))
        })
        .collect();
    let err = match &h.last_error {
        Some(e) => format!("\"{}\"", e.replace('\\', "\\\\").replace('"', "\\\"")),
        None => "null".into(),
    };
    set_result(format!(
        r#"{{"status":{},"retired":{},"rip":"{:#x}","rflags":"{:#x}","error":{},"registers":[{}],"data":[{}]}}"#,
        status(h),
        cpu.instructions_retired,
        cpu.regs.rip,
        cpu.regs.rflags,
        err,
        regs.join(","),
        data.join(",")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result() -> String {
        RESULT.with(|r| String::from_utf8(r.borrow().clone()).unwrap())
    }

    #[test]
    fn run_all_tests_via_c_abi() {
        let n = emu_run_all_tests();
        let s = result();
        assert_eq!(n, s.len());
        assert!(s.contains(r#""name":"add_and_store","#));
        assert!(!s.contains(r#""passed":false"#), "{s}");
    }

    #[test]
    fn single_step_milestone_program_via_c_abi() {
        unsafe {
            let h = emu_machine_new_test_program(0);
            assert!(!h.is_null());
            let mut steps = 0;
            while emu_machine_step(h) == EMU_RUNNING {
                steps += 1;
            }
            assert_eq!(emu_machine_step(h), EMU_HALTED);
            assert_eq!(steps, 6); // 7 instructions, last one (HLT) returns HALTED
            emu_machine_state_json(h);
            let s = result();
            assert!(s.contains(r#"["rax","0x000000000000000c"]"#), "{s}");
            assert!(s.contains(r#"["0x1000","0x000000000000000c"]"#), "{s}");
            emu_machine_free(h);
        }
    }

    #[test]
    fn error_is_reported_not_hidden() {
        unsafe {
            let h = emu_machine_new_test_program(3); // unsupported_cpuid
            assert_eq!(emu_machine_run(h), EMU_ERROR);
            emu_machine_state_json(h);
            assert!(result().contains("unsupported instruction (0F A2)"), "{}", result());
            emu_machine_free(h);
        }
    }

    #[test]
    fn alloc_roundtrip_and_oversized_code_rejected() {
        unsafe {
            let p = emu_alloc(0x2000);
            assert!(emu_machine_new(p, 0x2000).is_null()); // bigger than the code region
            emu_free(p, 0x2000);
        }
    }
}
