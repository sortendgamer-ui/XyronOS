//! page_table_entry.rs — a wrapper around the raw `u64` page-table
//! entry format.
//!
//! Bit layout (Present, Writable, physical address in bits 51:12,
//! No-Execute in bit 63) is dictated by the x86_64 architecture
//! itself (Intel SDM Vol. 3A Section 4.5, Table 4-19; AMD64 APM
//! Vol. 2 Section 5.3) — the same external hardware specification
//! `boot/paging.c` already implements independently for its own,
//! simpler (2 MiB huge-page-only) use. This file is the kernel-side
//! counterpart for arbitrary 4-level, 4-KiB-page tables.

use crate::mm::phys_addr::PhysAddr;

const FLAG_PRESENT: u64 = 1 << 0;
const FLAG_WRITABLE: u64 = 1 << 1;
/// Bit 63 — "Execute Disable" in Intel's naming, "No-Execute" (NX) in
/// AMD's. Only meaningful if `EFER.NXE` is set — see
/// `docs/kernel/MEMORY_MANAGER_DESIGN.md`'s "Concrete decisions"
/// section for why the VMM sets that MSR bit itself rather than
/// assuming firmware already did.
const FLAG_NO_EXECUTE: u64 = 1 << 63;

/// Bits 51:12 — the physical address of the frame (leaf entry) or
/// next-level table (non-leaf entry) this entry points to. Bits 63:52
/// and 11:0 are used for flags/reserved bits, not address bits, so
/// they're masked off both when reading and writing the address.
const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

/// Flags a caller of `VirtualMemoryManager::map` chooses for a new
/// mapping. `PRESENT` is not a caller-visible choice — every mapping
/// `map()` creates is present by definition; a "not present" entry is
/// simply the absence of a mapping (`unmap()`'s job), not a flag
/// combination `map()` would ever be asked to produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFlags {
    pub writable: bool,
    pub no_execute: bool,
}

impl PageFlags {
    /// Read-write, executable — the common case for kernel code/data
    /// this subsystem creates today (no read-only or execute-disabled
    /// mapping is created by anything in this milestone; both are
    /// implemented and available for the next caller that needs them).
    pub const fn read_write() -> Self {
        PageFlags {
            writable: true,
            no_execute: false,
        }
    }
}

/// One raw page-table entry, at any of the four levels (PML4E, PDPTE,
/// PDE, or PTE — this project's VMM only ever creates 4 KiB leaf
/// mappings, so every non-leaf entry here is a plain "pointer to the
/// next table," never a huge-page entry; huge pages are `boot/paging.c`'s
/// concern, not this one — see `docs/kernel/MEMORY_MANAGER_DESIGN.md`).
///
/// `#[repr(transparent)]`: guarantees this struct has IDENTICAL memory
/// layout to the single `u64` field it wraps — required for
/// `vmm.rs`'s `read_entry`/`write_entry` to `transmute` between a raw
/// `u64` read from a page-table page and this type; without this
/// guarantee, that transmute would be relying on unspecified default
/// struct layout.
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const fn is_present(self) -> bool {
        (self.0 & FLAG_PRESENT) != 0
    }

    /// Check an arbitrary raw bit against this entry — used by
    /// `vmm.rs` for the huge-page (PS, bit 7) check, which only
    /// applies to PDPTE/PDE entries and therefore has no named
    /// accessor of its own here (a `is_huge_page()` method on a type
    /// also used for plain PML4E/PTE entries, where that bit is
    /// reserved-must-be-zero rather than meaningful, would invite
    /// misuse at the wrong table level).
    pub const fn raw_has_flag(self, flag: u64) -> bool {
        (self.0 & flag) != 0
    }

    /// The physical address this entry points to — the next-level
    /// table for a non-leaf entry, or the mapped frame for a leaf
    /// entry. Meaningless (and not checked) if `is_present()` is
    /// false; callers must check presence first, matching how every
    /// walker function in `vmm.rs` uses this.
    pub const fn addr(self) -> PhysAddr {
        PhysAddr::new(self.0 & ADDR_MASK)
    }

    pub const fn is_writable(self) -> bool {
        (self.0 & FLAG_WRITABLE) != 0
    }

    pub const fn is_no_execute(self) -> bool {
        (self.0 & FLAG_NO_EXECUTE) != 0
    }

    /// Build a new present entry pointing at `addr`, for use as an
    /// intermediate table pointer (PML4E→PDPT, PDPTE→PD, PDE→PT).
    /// Always writable — an intermediate table's own permission bits
    /// don't restrict anything by themselves on x86_64 (the
    /// architecture ANDs permissions across every level down to the
    /// leaf), so the actual restriction belongs on the leaf entry
    /// `new_leaf` creates, not here.
    pub const fn new_table_pointer(addr: PhysAddr) -> Self {
        PageTableEntry((addr.as_u64() & ADDR_MASK) | FLAG_PRESENT | FLAG_WRITABLE)
    }

    /// Build a new present leaf entry (the final PTE mapping a 4 KiB
    /// frame) with the given flags.
    pub const fn new_leaf(addr: PhysAddr, flags: PageFlags) -> Self {
        let mut bits = (addr.as_u64() & ADDR_MASK) | FLAG_PRESENT;
        if flags.writable {
            bits |= FLAG_WRITABLE;
        }
        if flags.no_execute {
            bits |= FLAG_NO_EXECUTE;
        }
        PageTableEntry(bits)
    }

    /// The empty (not-present) entry `unmap()` writes back — distinct
    /// constructor from `empty()` only in name, kept separate so a
    /// call site reads as "clear this mapping" rather than "here is a
    /// blank starting value," even though the bit pattern is
    /// identical.
    pub const fn cleared() -> Self {
        PageTableEntry(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_pointer_round_trips_address() {
        let addr = PhysAddr::new(0x1234_5000);
        let entry = PageTableEntry::new_table_pointer(addr);
        assert!(entry.is_present());
        assert!(entry.is_writable());
        assert_eq!(entry.addr(), addr);
    }

    #[test]
    fn leaf_round_trips_address_and_flags() {
        let addr = PhysAddr::new(0xABCD_E000);
        let entry = PageTableEntry::new_leaf(
            addr,
            PageFlags {
                writable: false,
                no_execute: true,
            },
        );
        assert!(entry.is_present());
        assert!(!entry.is_writable());
        assert!(entry.is_no_execute());
        assert_eq!(entry.addr(), addr);
    }

    #[test]
    fn read_write_default_is_writable_and_executable() {
        let entry = PageTableEntry::new_leaf(PhysAddr::new(0x2000), PageFlags::read_write());
        assert!(entry.is_writable());
        assert!(!entry.is_no_execute());
    }

    #[test]
    fn address_mask_ignores_flag_bits() {
        // An address's low 12 bits (below FRAME_SIZE/page alignment)
        // must never leak into the flag bits, and the flag bits must
        // never leak into the reported address — this is the entire
        // correctness property ADDR_MASK exists to guarantee.
        let addr = PhysAddr::new(0x1_2345_6000);
        let entry = PageTableEntry::new_leaf(addr, PageFlags::read_write());
        assert_eq!(entry.addr(), addr);
    }

    #[test]
    fn cleared_is_not_present() {
        assert!(!PageTableEntry::cleared().is_present());
    }
}
