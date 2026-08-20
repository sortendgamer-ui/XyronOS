//! arch/x86_64/mod.rs — x86_64-specific kernel code.
//!
//! Early debug serial output (serial.rs), plus, as of the Interrupts
//! and Exceptions subsystem, the kernel's own GDT (gdt.rs), IDT
//! (idt.rs), and CPU exception handlers (exceptions.rs) — see
//! `docs/kernel/INTERRUPTS_DESIGN.md` for the full design.

pub mod exceptions;
pub mod gdt;
pub mod idt;
pub mod serial;
