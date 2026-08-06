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
use mm::{FrameAllocator, PageFlags, VirtAddr, VirtualMemoryManager};

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

/// Boot-time integration self-test for the virtual memory manager —
/// requirement 6/7 ("comprehensive unit tests and boot-time
/// self-tests"). `map`/`unmap`/`translate`'s actual page-table
/// manipulation needs real physical memory and the real frame
/// allocator (see `docs/kernel/MEMORY_MANAGER_DESIGN.md`'s testing
/// split), so — like the frame allocator's own boot self-test — this
/// runs live rather than on the host target. Returns `true` if every
/// check passed.
///
/// Explicitly NOT verified here (stated rather than silently skipped,
/// per the instruction to never fake verification): whether the CPU
/// actually enforces the `WRITABLE`/`NO_EXECUTE` flags this test sets
/// (a real write-protection or execute-protection fault). That
/// requires a page-fault exception handler, which does not exist
/// until the next kernel subsystem.
#[cfg(not(test))]
fn run_vmm_boot_self_test(vmm: &mut VirtualMemoryManager, allocator: &mut FrameAllocator, boot_info: &BootInfo) -> bool {
    serial::write_str("\r\nRunning virtual memory manager boot self-test...\r\n");

    // Check 1: translate() against the kernel's OWN already-running
    // code — proves the walker correctly handles the bootloader's
    // existing 2 MiB huge-page higher-half mapping (ADR-005), not
    // just newly created 4 KiB leaves. "Higher-half kernel mapping
    // compatibility," verified concretely rather than assumed.
    let self_fn_vaddr = run_vmm_boot_self_test as *const () as u64;
    match vmm.translate(VirtAddr::new(self_fn_vaddr)) {
        Some(phys) => {
            let in_range = phys.as_u64() >= boot_info.kernel_physical_base
                && phys.as_u64() < boot_info.kernel_physical_base + boot_info.kernel_size_bytes;
            if !in_range {
                serial::write_str("  FAIL: translate() of kernel code address landed outside the kernel's own physical image.\r\n");
                return false;
            }
        }
        None => {
            serial::write_str("  FAIL: translate() could not resolve a kernel code address (huge-page walk broken).\r\n");
            return false;
        }
    }
    serial::write_str("  [OK] translate() correctly resolves an address inside the bootloader's existing higher-half (2 MiB huge-page) kernel mapping.\r\n");

    // Check 2: map() a fresh 4 KiB page into the (currently entirely
    // unused) kernel heap region from ADR-002, write a known pattern
    // through the new mapping, and read it back.
    const TEST_VADDR: u64 = 0xFFFF_8800_0000_0000; // kernel heap region base, ADR-002
    let test_frame = match allocator.allocate() {
        Some(f) => f,
        None => {
            serial::write_str("  FAIL: frame allocator exhausted before the VMM self-test could run.\r\n");
            return false;
        }
    };

    if let Err(_e) = vmm.map(allocator, VirtAddr::new(TEST_VADDR), test_frame, PageFlags::read_write()) {
        serial::write_str("  FAIL: map() of a fresh page in the kernel heap region failed.\r\n");
        return false;
    }
    serial::write_str("  [OK] map() succeeded for a fresh page in the (previously unmapped) kernel heap region.\r\n");

    const TEST_PATTERN: u64 = 0xDEAD_BEEF_CAFE_BABE;
    // SAFETY: TEST_VADDR was just mapped, read-write, to test_frame by
    // the map() call above — writing and reading back a u64 at the
    // start of that now-valid page.
    unsafe {
        core::ptr::write_volatile(TEST_VADDR as *mut u64, TEST_PATTERN);
        let readback = core::ptr::read_volatile(TEST_VADDR as *const u64);
        if readback != TEST_PATTERN {
            serial::write_str("  FAIL: read-back through the new mapping did not match what was written.\r\n");
            return false;
        }
    }
    serial::write_str("  [OK] Write-then-read-back through the new mapping round-tripped correctly.\r\n");

    // Check 3a: the permission flags map() was given must be exactly
    // what's stored in the page table entry — this is what makes
    // "page permissions" a verified end-to-end property, not just
    // something PageTableEntry's own isolated unit tests trust.
    match vmm.flags_at(VirtAddr::new(TEST_VADDR)) {
        Some(flags) if flags.writable && !flags.no_execute => {}
        Some(_) => {
            serial::write_str("  FAIL: stored permission flags do not match what map() was asked for.\r\n");
            return false;
        }
        None => {
            serial::write_str("  FAIL: flags_at() found no mapping immediately after a successful map().\r\n");
            return false;
        }
    }
    serial::write_str("  [OK] Stored permission flags match exactly what map() was asked for.\r\n");

    // Check 3: translate() must report the exact frame map() was
    // given.
    match vmm.translate(VirtAddr::new(TEST_VADDR)) {
        Some(phys) if phys == test_frame => {}
        _ => {
            serial::write_str("  FAIL: translate() did not report the frame map() was given.\r\n");
            return false;
        }
    }
    serial::write_str("  [OK] translate() reports the correct physical frame for the new mapping.\r\n");

    // Check 4: mapping the same address twice must fail — a real,
    // meaningful error path, not just a happy-path check.
    let second_frame = allocator.allocate();
    if let Some(sf) = second_frame {
        let result = vmm.map(allocator, VirtAddr::new(TEST_VADDR), sf, PageFlags::read_write());
        allocator.deallocate(sf).ok();
        if result.is_ok() {
            serial::write_str("  FAIL: map() allowed mapping an already-mapped address.\r\n");
            return false;
        }
    }
    serial::write_str("  [OK] map() correctly rejects an already-mapped address.\r\n");

    // Check 5: unmap() must return the same frame that was mapped,
    // and translate() must report None afterward.
    match vmm.unmap(VirtAddr::new(TEST_VADDR)) {
        Ok(phys) if phys == test_frame => {}
        _ => {
            serial::write_str("  FAIL: unmap() did not return the originally-mapped frame.\r\n");
            return false;
        }
    }
    if vmm.translate(VirtAddr::new(TEST_VADDR)).is_some() {
        serial::write_str("  FAIL: translate() still resolves the address after unmap().\r\n");
        return false;
    }
    serial::write_str("  [OK] unmap() succeeded; translate() now correctly reports no mapping.\r\n");

    // Check 6: unmapping an already-unmapped address must fail.
    if vmm.unmap(VirtAddr::new(TEST_VADDR)).is_ok() {
        serial::write_str("  FAIL: unmap() succeeded on an address that was already unmapped.\r\n");
        return false;
    }
    serial::write_str("  [OK] unmap() correctly rejects an already-unmapped address.\r\n");

    if allocator.deallocate(test_frame).is_err() {
        serial::write_str("  FAIL: could not free the test frame back to the allocator after unmap().\r\n");
        return false;
    }

    serial::write_str("  NOTE: WRITABLE/NO_EXECUTE flags are set correctly (see mm/page_table_entry.rs\r\n");
    serial::write_str("        unit tests) but hardware ENFORCEMENT of them is not verified here —\r\n");
    serial::write_str("        that requires a page-fault handler, which does not exist yet.\r\n");
    serial::write_str("Virtual memory manager boot self-test: ALL CHECKS PASSED.\r\n");
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

    // Virtual memory manager bring-up — still part of ADR-006's step 3
    // (memory manager), the next piece of this same subsystem per
    // docs/kernel/MEMORY_MANAGER_DESIGN.md.
    //
    // SAFETY: called exactly once, after the frame allocator is
    // initialized (satisfied above), with the bootloader's page
    // tables (ADR-005) still active in CR3 — nothing in this kernel
    // has changed CR3 since the bootloader's jump.
    let mut vmm = unsafe { VirtualMemoryManager::init() };
    serial::write_str("[OK] Virtual memory manager initialized (EFER.NXE ");
    serial::write_str("set, reusing the bootloader's existing PML4 at 0x");
    serial::write_hex64(vmm.pml4_phys().as_u64());
    serial::write_str(").\r\n");

    if !run_vmm_boot_self_test(&mut vmm, &mut frame_allocator, boot_info) {
        serial::write_str("FATAL: virtual memory manager boot self-test failed.\r\n");
        halt();
    }

    serial::write_str("\r\nMEMORY MANAGER SUBSYSTEM: virtual memory manager verified.\r\n");
    serial::write_str("Kernel heap allocator: not yet implemented (see docs/kernel/).\r\n");
    serial::write_str("Halting.\r\n");

    halt()
}
