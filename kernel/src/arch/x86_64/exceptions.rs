//! exceptions.rs — the 32 CPU exception handlers (vectors 0-31) and
//! their diagnostic reporting. See `docs/kernel/INTERRUPTS_DESIGN.md`
//! for the full vector/error-code/name table this file implements
//! exactly, and for why breakpoint (vector 3) is the one handler that
//! returns instead of halting.
//!
//! Requires the unstable `abi_x86_interrupt` feature (enabled at the
//! crate root, `main.rs`) — this generates the correct
//! prologue/epilogue (saving registers the CPU doesn't save
//! automatically, using `iretq` rather than `ret`) for a function
//! that's genuinely invoked by CPU exception delivery, not an
//! ordinary call.

use crate::arch::x86_64::gdt::DOUBLE_FAULT_IST_INDEX;
use crate::arch::x86_64::idt::{self, IdtEntry};
use crate::arch::x86_64::serial;

/// The frame the CPU pushes automatically before invoking any
/// exception handler — layout fixed by the architecture (Intel SDM
/// Vol. 3A Section 6.14.2). Taken by value in every handler below,
/// per the `extern "x86-interrupt"` ABI's expectations.
#[repr(C)]
pub struct InterruptStackFrame {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

fn read_cr2() -> u64 {
    let value: u64;
    // SAFETY: reading CR2 has no side effects; valid at any time, but
    // only architecturally MEANINGFUL immediately after a page fault
    // (vector 14) — the only vector this is called for, before any
    // other memory access that could itself fault and overwrite it.
    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Decode the page-fault error code's individual bits (Intel SDM Vol.
/// 3A Section 4.7) — the raw hex value alone doesn't say whether this
/// was a read or write, present-but-protection-violation vs.
/// not-present, user-mode or supervisor-mode, or an instruction fetch,
/// and each of those changes what actually went wrong.
fn report_page_fault_error_code(error_code: u64) {
    serial::write_str("  Cause: ");
    serial::write_str(if error_code & 1 != 0 {
        "protection violation (page present)"
    } else {
        "non-present page"
    });
    serial::write_str(", ");
    serial::write_str(if error_code & 0b10 != 0 { "write" } else { "read" });
    serial::write_str(", ");
    serial::write_str(if error_code & 0b100 != 0 {
        "user-mode"
    } else {
        "supervisor-mode"
    });
    if error_code & 0b1000 != 0 {
        serial::write_str(", reserved-bit-set page table entry");
    }
    if error_code & 0b10000 != 0 {
        serial::write_str(", instruction fetch");
    }
    serial::write_str("\r\n");
}

/// Shared diagnostic reporting for every exception handler — prints
/// the vector, its name, the error code (decoded further for page
/// faults, per `docs/kernel/INTERRUPTS_DESIGN.md`), `CR2` for page
/// faults specifically, and the saved interrupt frame. Halts
/// afterward per ADR-006's panic policy, EXCEPT for breakpoint
/// (vector 3), which is architecturally meant to return control to
/// right after the triggering instruction — see the design doc for
/// why this is correct behavior, not a special case invented for
/// convenience.
fn report(vector: u8, name: &str, error_code: Option<u64>, frame: &InterruptStackFrame) {
    serial::write_str("\r\n!!! CPU EXCEPTION !!!\r\n");
    serial::write_str("Vector: 0x");
    serial::write_hex64(vector as u64);
    serial::write_str(" (");
    serial::write_str(name);
    serial::write_str(")\r\n");

    if let Some(code) = error_code {
        serial::write_str("Error code: 0x");
        serial::write_hex64(code);
        serial::write_str("\r\n");
        if vector == 14 {
            report_page_fault_error_code(code);
        }
    }

    if vector == 14 {
        serial::write_str("CR2 (faulting address): 0x");
        serial::write_hex64(read_cr2());
        serial::write_str("\r\n");
    }

    serial::write_str("RIP: 0x");
    serial::write_hex64(frame.rip);
    serial::write_str("  CS: 0x");
    serial::write_hex64(frame.cs);
    serial::write_str("\r\nRFLAGS: 0x");
    serial::write_hex64(frame.rflags);
    serial::write_str("\r\nRSP: 0x");
    serial::write_hex64(frame.rsp);
    serial::write_str("  SS: 0x");
    serial::write_hex64(frame.ss);
    serial::write_str("\r\n");

    if vector == 3 {
        serial::write_str("(Breakpoint — resuming execution.)\r\n");
        return;
    }

    serial::write_str("Kernel halted.\r\n");
    loop {
        // SAFETY: HLT with no side effects other than halting the CPU
        // — same idle primitive used throughout this kernel and the
        // bootloader before it.
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack));
        }
    }
}

/// Generates one `extern "x86-interrupt"` handler function per
/// no-error-code vector — each hardcodes its own vector number and
/// name (the CPU does not push a vector number for CPU exceptions,
/// unlike software `int n`, so there is no other way for a handler to
/// know which vector invoked it). Boilerplate generation, not a
/// shortcut: every generated function is real, distinct, and
/// correctly typed — see `docs/kernel/INTERRUPTS_DESIGN.md`.
macro_rules! handler_no_error {
    ($fn_name:ident, $vector:expr, $name:expr) => {
        extern "x86-interrupt" fn $fn_name(stack_frame: InterruptStackFrame) {
            report($vector, $name, None, &stack_frame);
        }
    };
}

macro_rules! handler_with_error {
    ($fn_name:ident, $vector:expr, $name:expr) => {
        extern "x86-interrupt" fn $fn_name(stack_frame: InterruptStackFrame, error_code: u64) {
            report($vector, $name, Some(error_code), &stack_frame);
        }
    };
}

handler_no_error!(handler_00, 0, "Divide Error (#DE)");
handler_no_error!(handler_01, 1, "Debug (#DB)");
handler_no_error!(handler_02, 2, "Non-Maskable Interrupt");
handler_no_error!(handler_03, 3, "Breakpoint (#BP)");
handler_no_error!(handler_04, 4, "Overflow (#OF)");
handler_no_error!(handler_05, 5, "Bound Range Exceeded (#BR)");
handler_no_error!(handler_06, 6, "Invalid Opcode (#UD)");
handler_no_error!(handler_07, 7, "Device Not Available (#NM)");
handler_with_error!(handler_08, 8, "Double Fault (#DF)");
handler_no_error!(handler_09, 9, "Reserved (legacy Coprocessor Segment Overrun)");
handler_with_error!(handler_10, 10, "Invalid TSS (#TS)");
handler_with_error!(handler_11, 11, "Segment Not Present (#NP)");
handler_with_error!(handler_12, 12, "Stack-Segment Fault (#SS)");
handler_with_error!(handler_13, 13, "General Protection Fault (#GP)");
handler_with_error!(handler_14, 14, "Page Fault (#PF)");
handler_no_error!(handler_15, 15, "Reserved");
handler_no_error!(handler_16, 16, "x87 Floating-Point Exception (#MF)");
handler_with_error!(handler_17, 17, "Alignment Check (#AC)");
handler_no_error!(handler_18, 18, "Machine Check (#MC)");
handler_no_error!(handler_19, 19, "SIMD Floating-Point Exception (#XM)");
handler_no_error!(handler_20, 20, "Virtualization Exception (#VE)");
handler_with_error!(handler_21, 21, "Control Protection Exception (#CP)");
handler_no_error!(handler_22, 22, "Reserved");
handler_no_error!(handler_23, 23, "Reserved");
handler_no_error!(handler_24, 24, "Reserved");
handler_no_error!(handler_25, 25, "Reserved");
handler_no_error!(handler_26, 26, "Reserved");
handler_no_error!(handler_27, 27, "Reserved");
handler_no_error!(handler_28, 28, "Hypervisor Injection Exception (#HV)");
handler_with_error!(handler_29, 29, "VMM Communication Exception (#VC)");
handler_with_error!(handler_30, 30, "Security Exception (#SX)");
handler_no_error!(handler_31, 31, "Reserved");

/// Install every one of the 32 generated handlers above into the IDT
/// — called once from `idt::init`. Vector 8 (double fault) is the
/// only entry given a nonzero IST index, per
/// `docs/kernel/INTERRUPTS_DESIGN.md`'s explanation of why double
/// fault specifically needs a dedicated, always-valid stack.
pub(super) fn install_handlers(idt: &mut [IdtEntry; 256]) {
    macro_rules! install {
        ($vector:expr, $fn_name:ident) => {
            idt::set_entry(idt, $vector, $fn_name as u64, 0)
        };
    }

    install!(0, handler_00);
    install!(1, handler_01);
    install!(2, handler_02);
    install!(3, handler_03);
    install!(4, handler_04);
    install!(5, handler_05);
    install!(6, handler_06);
    install!(7, handler_07);
    idt::set_entry(idt, 8, handler_08 as u64, DOUBLE_FAULT_IST_INDEX as u8);
    install!(9, handler_09);
    install!(10, handler_10);
    install!(11, handler_11);
    install!(12, handler_12);
    install!(13, handler_13);
    install!(14, handler_14);
    install!(15, handler_15);
    install!(16, handler_16);
    install!(17, handler_17);
    install!(18, handler_18);
    install!(19, handler_19);
    install!(20, handler_20);
    install!(21, handler_21);
    install!(22, handler_22);
    install!(23, handler_23);
    install!(24, handler_24);
    install!(25, handler_25);
    install!(26, handler_26);
    install!(27, handler_27);
    install!(28, handler_28);
    install!(29, handler_29);
    install!(30, handler_30);
    install!(31, handler_31);
}
