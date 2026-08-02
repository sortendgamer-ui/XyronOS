//! arch/x86_64/mod.rs — x86_64-specific kernel code.
//!
//! Currently only early debug serial output (see serial.rs for why
//! this exists and what it deliberately is not). `gdt`/`idt` modules
//! are added here when the interrupts/exceptions kernel subsystem
//! begins, per ADR-006's module layout — not created empty now.

pub mod serial;
