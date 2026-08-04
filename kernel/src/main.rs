//! main.rs — kernel entry point.
//!
//! Phase 3 skeleton (validate BootInfo, report over serial, halt) is
//! unchanged below except for the additions this subsystem needs:
//! `mod mm;`, memory manager bring-up (ADR-006 boot flow step 3), and
//! a `#[cfg(test)]`-gated split so unit tests (in mm/*.rs) can run on
//! the host target — see kernel/README.md for the exact command. The
//! freestanding `#![no_std]`/`#![no_main]`/panic handler only apply
//! to the real (non-test) kernel binary; a host `cargo test` run gets
//! ordinary std and the standard test harness instead.
//!
//! Reached via `boot/trampoline.asm`'s `jmp rdx`, with RDI holding a
//! physical pointer to a populated `BootInfo` (System V AMD64 ABI,
//! ADR-005), already running on the dedicated stack and page tables
//! the bootloader built.

#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
// Under `cfg(test)`, kernel_main and everything it calls are cfg'd
// out (see below), which leaves most of mm/'s public API legitimately
// unreachable from unit tests alone — they exercise the bitmap logic
// directly, not through kernel_main's call chain. The real
// (non-test) build enforces dead_code normally; it is verified clean
// (see kernel/README.md's build commands) and this allow does not
// apply to it.
#![cfg_attr(test, allow(dead_code))]

mod arch;
mod boot_info;
mod mm;

#[cfg_attr(test, allow(unused_imports))]
use arch::x86_64::serial;
#[cfg(not(test))]
use boot_info::BootInfo;
#[cfg(not(test))]
use mm::FrameAllocator;

/// Halt forever via `hlt` — the same idle primitive
/// `boot/main.c`/`tests/kernel_stub/kernel_stub.c` both use, not a
/// placeholder busy-loop; a correct way to stop without a scheduler
/// to hand control to yet.
#[cfg(not(test))]
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

#[cfg(not(test))]
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

/// Boot-time integration self-test for the physical frame allocator —
/// requirement 7 ("integration tests that verify allocator
/// correctness during boot"), distinct from the unit tests in
/// `mm/frame_allocator.rs` (which test the bitmap logic in isolation
/// on the host target). This test exercises the allocator built from
/// the REAL UEFI memory map this specific boot received — something a
/// host-run unit test structurally cannot do, since it has no real
/// `BootInfo` to parse. Returns `true` if every check passed.
#[cfg(not(test))]
fn run_frame_allocator_boot_self_test(allocator: &mut FrameAllocator) -> bool {
    serial::write_str("\r\nRunning frame allocator boot self-test...\r\n");

    let free_before = allocator.frames_free();
    if free_before == 0 {
        serial::write_str("  FAIL: zero free frames reported — memory map parsing likely broken.\r\n");
        return false;
    }
    serial::write_str("  [OK] Free frames after init: 0x");
    serial::write_hex64(free_before as u64);
    serial::write_str("\r\n");

    // Allocate a handful of frames and confirm every one is distinct
    // — the single most important correctness property a frame
    // allocator has: no two live allocations may ever alias.
    const TEST_COUNT: usize = 16;
    let mut allocated = [0u64; TEST_COUNT];
    for slot in allocated.iter_mut() {
        match allocator.allocate() {
            Some(addr) => *slot = addr.as_u64(),
            None => {
                serial::write_str("  FAIL: allocate() returned None unexpectedly early.\r\n");
                return false;
            }
        }
    }
    for i in 0..TEST_COUNT {
        for j in (i + 1)..TEST_COUNT {
            if allocated[i] == allocated[j] {
                serial::write_str("  FAIL: allocate() returned the same frame twice.\r\n");
                return false;
            }
        }
    }
    serial::write_str("  [OK] 16 allocations, all frames distinct.\r\n");

    if allocator.frames_free() != free_before - TEST_COUNT {
        serial::write_str("  FAIL: frames_free() bookkeeping incorrect after allocation.\r\n");
        return false;
    }
    serial::write_str("  [OK] frames_free() bookkeeping correct after allocation.\r\n");

    // Free every allocated frame and confirm the count returns to
    // exactly where it started — verifies deallocate() and the
    // is_free/mark_free bookkeeping symmetrically to the allocate path
    // above.
    for &addr in allocated.iter() {
        if allocator
            .deallocate(mm::PhysAddr::new(addr))
            .is_err()
        {
            serial::write_str("  FAIL: deallocate() rejected a frame this test just allocated.\r\n");
            return false;
        }
    }
    if allocator.frames_free() != free_before {
        serial::write_str("  FAIL: frames_free() did not return to its pre-test value after freeing everything.\r\n");
        return false;
    }
    serial::write_str("  [OK] All 16 frames freed; frames_free() returned to its original value.\r\n");

    serial::write_str("Frame allocator boot self-test: ALL CHECKS PASSED.\r\n");
    true
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn kernel_main(boot_info_ptr: *const BootInfo) -> ! {
    // Step 1 (ADR-006 boot flow): early debug output, so every step
    // after this can report failure somewhere observable.
    serial::init();
    serial::write_str("\r\n================================================\r\n");
    serial::write_str("XyronOS Kernel - Memory Manager subsystem\r\n");
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
    // anything else in it.
    if !boot_info.is_valid() {
        serial::write_str("FATAL: BootInfo validation failed (magic/version/size).\r\n");
        halt();
    }
    serial::write_str("[OK] BootInfo magic, version, and size validated.\r\n");

    serial::write_str("Memory map entries     : 0x");
    serial::write_hex64(boot_info.memory_map_entry_count);
    serial::write_str("\r\nMemory map desc. size  : 0x");
    serial::write_hex64(boot_info.memory_map_descriptor_size);
    serial::write_str("\r\n");

    // Step 3 (ADR-006 boot flow): memory manager bring-up. Only the
    // physical frame allocator exists so far — the virtual memory
    // manager and kernel heap remain future subsystem work per
    // docs/kernel/MEMORY_MANAGER_DESIGN.md.
    //
    // SAFETY: BootInfo was just validated above; this is the first
    // and only call to FrameAllocator::init this boot; the
    // bootloader's identity-mapped page tables (ADR-005) are still
    // active, matching every precondition FrameAllocator::init
    // documents.
    let mut frame_allocator = match unsafe { FrameAllocator::init(boot_info) } {
        Ok(alloc) => alloc,
        Err(e) => {
            serial::write_str("FATAL: FrameAllocator::init failed: ");
            serial::write_str(match e {
                mm::frame_allocator::InitError::InconsistentMemoryMap => "InconsistentMemoryMap",
                mm::frame_allocator::InitError::NoSuitableBitmapRegion => "NoSuitableBitmapRegion",
                mm::frame_allocator::InitError::ImpliedAddressSpaceTooLarge => {
                    "ImpliedAddressSpaceTooLarge"
                }
            });
            serial::write_str("\r\n");
            halt();
        }
    };
    serial::write_str("[OK] Physical frame allocator initialized.\r\n");
    serial::write_str("  Total frames : 0x");
    serial::write_hex64(frame_allocator.total_frames() as u64);
    serial::write_str("\r\n  Free frames  : 0x");
    serial::write_hex64(frame_allocator.frames_free() as u64);
    serial::write_str("\r\n");

    // Requirement 7: integration test verifying allocator correctness
    // against the REAL memory map this boot received, not just the
    // synthetic bitmaps the host-run unit tests use.
    if !run_frame_allocator_boot_self_test(&mut frame_allocator) {
        serial::write_str("FATAL: frame allocator boot self-test failed.\r\n");
        halt();
    }

    serial::write_str("\r\nMEMORY MANAGER SUBSYSTEM: physical frame allocator verified.\r\n");
    serial::write_str("Virtual memory manager, kernel heap: not yet implemented (see docs/kernel/).\r\n");
    serial::write_str("Halting.\r\n");

    halt()
}
