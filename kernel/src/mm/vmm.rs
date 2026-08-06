//! vmm.rs — the virtual memory manager: `map`, `unmap`, `translate`,
//! and the 4-level page-table walker underneath them.
//!
//! Reuses the same PML4 the bootloader built and activated (ADR-005)
//! — read once from CR3 at `init()`, never switched again here. New
//! mappings extend that same table structure; the bootloader's
//! identity map (PML4[0]) and higher-half kernel mapping (PML4[511])
//! are never modified by anything in this file. See
//! `docs/kernel/MEMORY_MANAGER_DESIGN.md`'s "Concrete decisions"
//! section for the design reasoning behind every non-obvious choice
//! below (EFER.NXE, TLB invalidation, the `IDENTITY_MAP_LIMIT` check).

use crate::mm::frame_allocator::{FrameAllocator, IDENTITY_MAP_LIMIT};
use crate::mm::page_table_entry::{PageFlags, PageTableEntry};
use crate::mm::phys_addr::PhysAddr;
use crate::mm::virt_addr::VirtAddr;

const EFER_MSR: u32 = 0xC000_0080;
const EFER_NXE_BIT: u64 = 1 << 11;

/// Bit 7 in a PDPTE or PDE — "this entry maps a huge page directly
/// (1 GiB or 2 MiB respectively), it is not a pointer to the next
/// table." `boot/paging.c` sets this for every entry it creates (its
/// identity map and higher-half kernel mapping are 2 MiB pages
/// throughout); this VMM's own `map()` never sets it (every mapping
/// this subsystem creates is a 4 KiB leaf per
/// `docs/kernel/MEMORY_MANAGER_DESIGN.md`), but the walker must
/// recognize it when it encounters one of the bootloader's existing
/// entries — most importantly for `translate()` to correctly report
/// addresses inside the kernel's own higher-half image.
const FLAG_PAGE_SIZE: u64 = 1 << 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    /// `virt` or `phys` was not 4 KiB-aligned.
    NotPageAligned,
    /// A mapping already exists at this virtual address.
    AlreadyMapped,
    /// The frame allocator had no free frame below `IDENTITY_MAP_LIMIT`
    /// to use as a new intermediate page-table page.
    OutOfFrames,
    /// A page-table pointer encountered mid-walk pointed above
    /// `IDENTITY_MAP_LIMIT` — see `TECH_DEBT.md`'s entry on
    /// `boot/paging.c`'s unconstrained page-table allocations. Never
    /// observed in practice; checked for and rejected rather than
    /// assumed impossible.
    PageTableUnreachable,
    /// The walk reached an existing huge-page (2 MiB or 1 GiB) entry
    /// partway through — `map()` cannot safely subdivide an existing
    /// huge page into 4 KiB entries (out of scope for this
    /// milestone), so it refuses rather than corrupting the mapping.
    WouldOverlapHugePage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnmapError {
    NotPageAligned,
    /// No mapping exists at this virtual address.
    NotMapped,
    PageTableUnreachable,
    /// The address falls within an existing huge-page mapping (e.g.
    /// the kernel's own higher-half image) — `unmap()` only ever
    /// removes 4 KiB leaf entries this subsystem itself created, never
    /// a huge page it didn't create.
    IsHugePage,
}

/// SAFETY (module-wide): every function in this file that
/// dereferences a physical address as a page-table pointer first
/// checks it against `IDENTITY_MAP_LIMIT` via `checked_table_ptr` —
/// the one place that check lives, so every caller inherits it
/// automatically rather than needing to remember it individually.
fn checked_table_ptr(phys: PhysAddr) -> Result<*mut u64, ()> {
    if phys.as_u64() >= IDENTITY_MAP_LIMIT {
        return Err(());
    }
    // SAFETY: phys is below IDENTITY_MAP_LIMIT, which the bootloader's
    // identity map (boot/paging.c, ADR-005) covers 1:1 — virtual
    // address == physical address for this range under the page
    // tables currently active (the same ones this module reads from
    // CR3 in `init` and never replaces).
    Ok(phys.as_u64() as *mut u64)
}

/// # Safety
/// Caller must ensure `ptr` is a valid, currently-mapped pointer to a
/// 4 KiB page-table page (guaranteed by every call site in this file
/// going through `checked_table_ptr` first) and that `index < 512`.
unsafe fn read_entry(ptr: *mut u64, index: usize) -> PageTableEntry {
    core::mem::transmute::<u64, PageTableEntry>(core::ptr::read_volatile(ptr.add(index)))
}

/// # Safety
/// Same preconditions as `read_entry`.
unsafe fn write_entry(ptr: *mut u64, index: usize, entry: PageTableEntry) {
    core::ptr::write_volatile(ptr.add(index), core::mem::transmute::<PageTableEntry, u64>(entry));
}

/// # Safety
/// `ptr` must point to a full, exclusively-owned 4 KiB page — true for
/// every table page this file allocates itself (freshly obtained from
/// `FrameAllocator::allocate_below`, never shared before this call).
unsafe fn zero_table(ptr: *mut u64) {
    for i in 0..512 {
        core::ptr::write_volatile(ptr.add(i), 0);
    }
}

fn read_cr3() -> u64 {
    let value: u64;
    // SAFETY: reading CR3 has no side effects and is always valid
    // from ring 0, which this kernel runs at exclusively (ADR-006:
    // monolithic kernel, no ring-3 code exists yet).
    unsafe {
        core::arch::asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// # Safety
/// Caller must ensure `addr` was previously mapped or unmapped by a
/// page-table modification that has already completed (the `invlpg`
/// must come AFTER the table write it is invalidating for, never
/// before) — every call site in this file satisfies this by
/// construction.
unsafe fn invlpg(addr: u64) {
    core::arch::asm!("invlpg [{}]", in(reg) addr, options(nostack, preserves_flags));
}

fn read_efer() -> u64 {
    let (low, high): (u32, u32);
    // SAFETY: RDMSR on EFER (a well-defined, always-present MSR on
    // any x86_64 CPU running in long mode, which this kernel already
    // is by the time it runs at all) has no side effects.
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") EFER_MSR,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags),
        );
    }
    ((high as u64) << 32) | (low as u64)
}

/// # Safety
/// Caller must ensure `value` is a valid EFER value — in particular,
/// this file only ever sets the NXE bit on top of whatever firmware
/// already configured (read-modify-write in `enable_nxe`), never
/// clears bits or constructs a value from scratch, since EFER also
/// controls LME/LMA (long mode itself) and getting those wrong would
/// be catastrophic.
unsafe fn write_efer(value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    core::arch::asm!(
        "wrmsr",
        in("ecx") EFER_MSR,
        in("eax") low,
        in("edx") high,
        options(nomem, nostack, preserves_flags),
    );
}

/// Set EFER.NXE if not already set. Idempotent — a no-op if firmware
/// already enabled it. See `docs/kernel/MEMORY_MANAGER_DESIGN.md` for
/// why this cannot be assumed instead of checked/set.
fn enable_nxe() {
    let current = read_efer();
    if current & EFER_NXE_BIT == 0 {
        // SAFETY: read-modify-write preserving every other bit,
        // setting only NXE — see write_efer's safety comment.
        unsafe {
            write_efer(current | EFER_NXE_BIT);
        }
    }
}

pub struct VirtualMemoryManager {
    pml4_phys: PhysAddr,
}

impl VirtualMemoryManager {
    /// # Safety
    /// Must be called at most once, after the frame allocator is
    /// initialized (page-table pages this struct allocates later come
    /// from it), and with the bootloader's page tables (ADR-005)
    /// still active in CR3 — true at kernel boot before anything else
    /// changes CR3, which nothing in this kernel does yet.
    pub unsafe fn init() -> Self {
        enable_nxe();
        VirtualMemoryManager {
            pml4_phys: PhysAddr::new(read_cr3()),
        }
    }

    pub fn pml4_phys(&self) -> PhysAddr {
        self.pml4_phys
    }

    /// Walk to (creating if absent) the next-level table pointed to by
    /// entry `index` of the table at `table_phys`. Used by `map()` for
    /// the three intermediate levels (PML4→PDPT, PDPT→PD, PD→PT) —
    /// identical logic at each level since x86_64's non-leaf table
    /// format is the same at every level above the leaf.
    fn get_or_create_table(
        &mut self,
        allocator: &mut FrameAllocator,
        table_phys: PhysAddr,
        index: usize,
    ) -> Result<PhysAddr, MapError> {
        let ptr = checked_table_ptr(table_phys).map_err(|_| MapError::PageTableUnreachable)?;

        // SAFETY: ptr came from checked_table_ptr (identity-mapped,
        // valid page-table page per the module-wide safety note);
        // index < 512 always, since VirtAddr's index methods mask to
        // 9 bits (0..=511) by construction.
        let entry = unsafe { read_entry(ptr, index) };

        if entry.is_present() {
            if entry_is_huge_page(ptr, index) {
                return Err(MapError::WouldOverlapHugePage);
            }
            return Ok(entry.addr());
        }

        let new_table_phys = allocator
            .allocate_below(PhysAddr::new(IDENTITY_MAP_LIMIT))
            .ok_or(MapError::OutOfFrames)?;

        let new_table_ptr =
            checked_table_ptr(new_table_phys).map_err(|_| MapError::PageTableUnreachable)?;
        // SAFETY: new_table_phys was just allocated exclusively for
        // this purpose by allocate_below and has never been written
        // to before — zeroing a fresh, exclusively-owned page.
        unsafe {
            zero_table(new_table_ptr);
        }

        let new_entry = PageTableEntry::new_table_pointer(new_table_phys);
        // SAFETY: ptr valid per above; writing a fresh table pointer
        // into a previously-not-present entry.
        unsafe {
            write_entry(ptr, index, new_entry);
        }

        Ok(new_table_phys)
    }

    /// Map `virt` (4 KiB, must be page-aligned) to `phys` (must be
    /// frame-aligned) with the given permissions. Creates any
    /// intermediate PDPT/PD/PT tables that don't already exist yet,
    /// allocating them from `allocator`. Fails (without partially
    /// applying the mapping — every intermediate table created before
    /// a failure remains, harmlessly unused, rather than being rolled
    /// back; see `docs/kernel/MEMORY_MANAGER_DESIGN.md` for why this
    /// is an accepted simplification) if `virt` is already mapped, if
    /// alignment is wrong, or if the walk cannot proceed
    /// (`PageTableUnreachable`/`WouldOverlapHugePage`/`OutOfFrames`).
    pub fn map(
        &mut self,
        allocator: &mut FrameAllocator,
        virt: VirtAddr,
        phys: PhysAddr,
        flags: PageFlags,
    ) -> Result<(), MapError> {
        if !virt.is_page_aligned() || !phys.is_frame_aligned() {
            return Err(MapError::NotPageAligned);
        }

        let pdpt_phys = self.get_or_create_table(allocator, self.pml4_phys, virt.pml4_index())?;
        let pd_phys = self.get_or_create_table(allocator, pdpt_phys, virt.pdpt_index())?;
        let pt_phys = self.get_or_create_table(allocator, pd_phys, virt.pd_index())?;

        let pt_ptr = checked_table_ptr(pt_phys).map_err(|_| MapError::PageTableUnreachable)?;
        // SAFETY: pt_ptr valid per checked_table_ptr; pt_index() < 512.
        let existing = unsafe { read_entry(pt_ptr, virt.pt_index()) };
        if existing.is_present() {
            return Err(MapError::AlreadyMapped);
        }

        let new_entry = PageTableEntry::new_leaf(phys, flags);
        // SAFETY: pt_ptr valid; writing a fresh leaf entry into a
        // previously-not-present slot.
        unsafe {
            write_entry(pt_ptr, virt.pt_index(), new_entry);
            // Must happen AFTER the table write above: invalidates any
            // stale cached translation for this address so the CPU
            // uses the mapping just created, not whatever (if
            // anything) it had cached from before.
            invlpg(virt.as_u64());
        }

        Ok(())
    }

    /// Remove the 4 KiB mapping at `virt`, returning the physical
    /// frame it was mapped to (the caller — not this function — is
    /// responsible for deciding whether to free that frame back to
    /// the allocator; `unmap()` only ever undoes what `map()` did to
    /// the page table, mirroring the split already used for the
    /// bootloader's own resource cleanup pattern in `boot/main.c`).
    pub fn unmap(&mut self, virt: VirtAddr) -> Result<PhysAddr, UnmapError> {
        if !virt.is_page_aligned() {
            return Err(UnmapError::NotPageAligned);
        }

        let pml4_ptr =
            checked_table_ptr(self.pml4_phys).map_err(|_| UnmapError::PageTableUnreachable)?;
        // SAFETY: pml4_ptr valid; index < 512.
        let pml4e = unsafe { read_entry(pml4_ptr, virt.pml4_index()) };
        if !pml4e.is_present() {
            return Err(UnmapError::NotMapped);
        }

        let pdpt_ptr =
            checked_table_ptr(pml4e.addr()).map_err(|_| UnmapError::PageTableUnreachable)?;
        let pdpte = unsafe { read_entry(pdpt_ptr, virt.pdpt_index()) };
        if !pdpte.is_present() {
            return Err(UnmapError::NotMapped);
        }
        if pdpte.raw_has_flag(FLAG_PAGE_SIZE) {
            return Err(UnmapError::IsHugePage);
        }

        let pd_ptr =
            checked_table_ptr(pdpte.addr()).map_err(|_| UnmapError::PageTableUnreachable)?;
        let pde = unsafe { read_entry(pd_ptr, virt.pd_index()) };
        if !pde.is_present() {
            return Err(UnmapError::NotMapped);
        }
        if pde.raw_has_flag(FLAG_PAGE_SIZE) {
            return Err(UnmapError::IsHugePage);
        }

        let pt_ptr = checked_table_ptr(pde.addr()).map_err(|_| UnmapError::PageTableUnreachable)?;
        let pte = unsafe { read_entry(pt_ptr, virt.pt_index()) };
        if !pte.is_present() {
            return Err(UnmapError::NotMapped);
        }

        let mapped_phys = pte.addr();
        // SAFETY: pt_ptr valid; clearing a present entry back to
        // not-present.
        unsafe {
            write_entry(pt_ptr, virt.pt_index(), PageTableEntry::cleared());
            // Must happen AFTER the table write — see map()'s
            // identical reasoning.
            invlpg(virt.as_u64());
        }

        Ok(mapped_phys)
    }

    /// Read-only walk: what physical address does `virt` currently map
    /// to, if any? Correctly handles the bootloader's own existing 2
    /// MiB and (defensively, though `boot/paging.c` doesn't currently
    /// use them) 1 GiB huge-page entries, not just 4 KiB leaves this
    /// subsystem's own `map()` creates — required for "higher-half
    /// kernel mapping compatibility": translating an address inside
    /// the kernel's own already-running code must work correctly.
    pub fn translate(&self, virt: VirtAddr) -> Option<PhysAddr> {
        let pml4_ptr = checked_table_ptr(self.pml4_phys).ok()?;
        // SAFETY: pml4_ptr valid; index < 512.
        let pml4e = unsafe { read_entry(pml4_ptr, virt.pml4_index()) };
        if !pml4e.is_present() {
            return None;
        }

        let pdpt_ptr = checked_table_ptr(pml4e.addr()).ok()?;
        let pdpte = unsafe { read_entry(pdpt_ptr, virt.pdpt_index()) };
        if !pdpte.is_present() {
            return None;
        }
        if pdpte.raw_has_flag(FLAG_PAGE_SIZE) {
            // 1 GiB huge page: bits 51:30 are the frame base, bits
            // 29:0 are the offset within it.
            let base = pdpte.addr().as_u64() & !0x3FFF_FFFF;
            return Some(PhysAddr::new(base | (virt.as_u64() & 0x3FFF_FFFF)));
        }

        let pd_ptr = checked_table_ptr(pdpte.addr()).ok()?;
        let pde = unsafe { read_entry(pd_ptr, virt.pd_index()) };
        if !pde.is_present() {
            return None;
        }
        if pde.raw_has_flag(FLAG_PAGE_SIZE) {
            // 2 MiB huge page — exactly what boot/paging.c's identity
            // map and higher-half kernel mapping both use throughout.
            let base = pde.addr().as_u64() & !0x1F_FFFF;
            return Some(PhysAddr::new(base | (virt.as_u64() & 0x1F_FFFF)));
        }

        let pt_ptr = checked_table_ptr(pde.addr()).ok()?;
        let pte = unsafe { read_entry(pt_ptr, virt.pt_index()) };
        if !pte.is_present() {
            return None;
        }

        Some(PhysAddr::new(pte.addr().as_u64() | virt.page_offset()))
    }

    /// Read-only walk: what permission flags does the CURRENT mapping
    /// at `virt` have, if any? Distinct from `translate()` (which
    /// reports the physical address) so a caller that only cares
    /// about permissions doesn't need to separately re-derive them
    /// from a raw entry — used by the boot self-test to verify `map()`
    /// actually stored the requested flags end-to-end, not just that
    /// `PageTableEntry::new_leaf` (unit-tested in isolation) computes
    /// the right bits.
    pub fn flags_at(&self, virt: VirtAddr) -> Option<PageFlags> {
        let pml4_ptr = checked_table_ptr(self.pml4_phys).ok()?;
        // SAFETY: pml4_ptr valid; index < 512.
        let pml4e = unsafe { read_entry(pml4_ptr, virt.pml4_index()) };
        if !pml4e.is_present() {
            return None;
        }

        let pdpt_ptr = checked_table_ptr(pml4e.addr()).ok()?;
        let pdpte = unsafe { read_entry(pdpt_ptr, virt.pdpt_index()) };
        if !pdpte.is_present() || pdpte.raw_has_flag(FLAG_PAGE_SIZE) {
            // Huge-page permissions aren't this function's concern —
            // callers asking about a specific 4 KiB mapping this
            // subsystem created will never hit this path; only
            // translate() needs to handle huge pages, for the
            // higher-half-compatibility check.
            return None;
        }

        let pd_ptr = checked_table_ptr(pdpte.addr()).ok()?;
        let pde = unsafe { read_entry(pd_ptr, virt.pd_index()) };
        if !pde.is_present() || pde.raw_has_flag(FLAG_PAGE_SIZE) {
            return None;
        }

        let pt_ptr = checked_table_ptr(pde.addr()).ok()?;
        let pte = unsafe { read_entry(pt_ptr, virt.pt_index()) };
        if !pte.is_present() {
            return None;
        }

        Some(PageFlags {
            writable: pte.is_writable(),
            no_execute: pte.is_no_execute(),
        })
    }
}

/// Helper for `get_or_create_table`: re-reads the entry at `index` in
/// the table at `ptr` and checks the huge-page bit. Kept as a tiny
/// free function (rather than inlined) so its one call site stays
/// readable — "is this a huge page" reads clearly as a named check.
fn entry_is_huge_page(ptr: *mut u64, index: usize) -> bool {
    // SAFETY: caller (get_or_create_table) already validated ptr via
    // checked_table_ptr and index < 512 before calling this.
    let entry = unsafe { read_entry(ptr, index) };
    entry.raw_has_flag(FLAG_PAGE_SIZE)
}
