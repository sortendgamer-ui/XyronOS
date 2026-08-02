//! main.rs — kernel entry point, Phase 3 skeleton milestone.
//!
//! This is the Rust kernel's equivalent of Phase 2 Part 1: the
//! smallest program that proves the whole chain works end to end —
//! the custom target spec, `build-std`, the linker script placing the
//! kernel at the correct higher-half address, and the `BootInfo` ABI
//! contract (ADR-005) — before any subsystem with real design
//! complexity (memory manager, scheduler; see docs/kernel/) is built
//! on top of an unverified foundation. See ADR-006's "Boot flow and
//! initialization order" for exactly which steps this skeleton
//! performs (1-2) and which it deliberately does not (3 onward).
//!
//! Reached via `boot/trampoline.asm`'s `jmp rdx`, with RDI holding a
//! physical pointer to a populated `BootInfo` (System V AMD64 ABI,
//! ADR-005), already running on the dedicated stack and page tables
//! the bootloader built.

#![no_std]
#![no_main]

mod arch;
mod boot_info;

use arch::x86_64::serial;
use boot_info::BootInfo;

/// Halt forever via `hlt` — the same idle primitive
/// `boot/main.c`/`tests/kernel_stub/kernel_stub.c` both use, not a
/// placeholder busy-loop; a correct way to stop without a scheduler
/// to hand control to yet.
fn halt() -> ! {
    loop {
        // SAFETY: HLT with no side effects other than halting the CPU
        // until the next interrupt; sound to execute unconditionally
        // here since there is nothing else for this code path to do.
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack));
        }
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Per ADR-006's panic policy: report over the early debug serial
    // output, then halt unconditionally — no recovery model exists
    // yet (no process isolation this early), so every panic is fatal.
    serial::write_str("\r\n!!! KERNEL PANIC !!!\r\n");
    if let Some(location) = info.location() {
        serial::write_str("Location: ");
        serial::write_str(location.file());
        serial::write_str(":");
        serial::write_hex64(location.line() as u64);
        serial::write_str("\r\n");
    }
    halt()
}

#[no_mangle]
pub extern "C" fn kernel_main(boot_info_ptr: *const BootInfo) -> ! {
    // Step 1 (ADR-006 boot flow): early debug output, so every step
    // after this can report failure somewhere observable.
    serial::init();
    serial::write_str("\r\n================================================\r\n");
    serial::write_str("XyronOS Kernel - Phase 3 skeleton\r\n");
    serial::write_str("================================================\r\n");

    if boot_info_ptr.is_null() {
        serial::write_str("FATAL: BootInfo pointer is null.\r\n");
        halt();
    }

    // SAFETY: trampoline.asm passes a physical address that falls
    // within the identity-mapped region the bootloader's page tables
    // cover (ADR-005), and those same page tables are still active —
    // nothing has changed CR3 since the bootloader's jump. The
    // pointer is therefore valid to dereference as long as it is
    // non-null (checked above) and the bootloader populated it
    // correctly, which is exactly what is_valid() below verifies
    // before any field is trusted.
    let boot_info: &BootInfo = unsafe { &*boot_info_ptr };

    // Step 2 (ADR-006 boot flow): validate BootInfo before trusting
    // anything else in it — the same three checks
    // tests/kernel_stub/kernel_stub.c already proved work correctly
    // from the bootloader side.
    if !boot_info.is_valid() {
        serial::write_str("FATAL: BootInfo validation failed (magic/version/size).\r\n");
        halt();
    }
    serial::write_str("[OK] BootInfo magic, version, and size validated.\r\n\r\n");

    serial::write_str("Kernel physical base   : 0x");
    serial::write_hex64(boot_info.kernel_physical_base);
    serial::write_str("\r\nKernel virtual base    : 0x");
    serial::write_hex64(boot_info.kernel_virtual_base);
    serial::write_str("\r\nKernel size (bytes)    : 0x");
    serial::write_hex64(boot_info.kernel_size_bytes);
    serial::write_str("\r\nKernel stack top       : 0x");
    serial::write_hex64(boot_info.kernel_stack_top);
    serial::write_str("\r\nKernel stack size      : 0x");
    serial::write_hex64(boot_info.kernel_stack_size_bytes);
    serial::write_str("\r\nMemory map entries     : 0x");
    serial::write_hex64(boot_info.memory_map_entry_count);
    serial::write_str("\r\nMemory map desc. size  : 0x");
    serial::write_hex64(boot_info.memory_map_descriptor_size);
    serial::write_str("\r\n\r\n");

    serial::write_str("PHASE 3 SKELETON: kernel_main reached, BootInfo valid, Rust toolchain verified.\r\n");
    serial::write_str("Memory manager, interrupts, scheduler: not yet implemented (see docs/kernel/).\r\n");
    serial::write_str("Halting.\r\n");

    halt()
}
