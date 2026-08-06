//! virt_addr.rs — a newtype for virtual addresses, mirroring
//! `phys_addr.rs`'s `PhysAddr` for the same reason: the compiler
//! should catch a physical/virtual address mix-up everywhere, not
//! just where a human reviewer happens to notice one.
//!
//! Also owns the one piece of pure arithmetic every level of the
//! virtual memory manager depends on: decomposing an address into its
//! four 9-bit page-table indices (PML4/PDPT/PD/PT) plus a 12-bit page
//! offset. This split — 9/9/9/9/12 bits — is the x86_64 architecture's
//! 4-level paging layout (Intel SDM Vol. 3A, Section 4.5; AMD64 APM
//! Vol. 2, Section 5.3), a hardware fact, not a design choice.

use crate::mm::phys_addr::FRAME_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtAddr(u64);

impl VirtAddr {
    pub const fn new(addr: u64) -> Self {
        VirtAddr(addr)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// True if this address is exactly page-aligned (4 KiB, the same
    /// `FRAME_SIZE` the physical side uses — the two are always equal
    /// on x86_64 since a page and a frame are the same size).
    pub const fn is_page_aligned(self) -> bool {
        self.0 % FRAME_SIZE == 0
    }

    /// PML4 table index — bits 47:39.
    pub const fn pml4_index(self) -> usize {
        ((self.0 >> 39) & 0x1FF) as usize
    }

    /// PDPT (page-directory-pointer table) index — bits 38:30.
    pub const fn pdpt_index(self) -> usize {
        ((self.0 >> 30) & 0x1FF) as usize
    }

    /// PD (page directory) index — bits 29:21.
    pub const fn pd_index(self) -> usize {
        ((self.0 >> 21) & 0x1FF) as usize
    }

    /// PT (page table) index — bits 20:12.
    pub const fn pt_index(self) -> usize {
        ((self.0 >> 12) & 0x1FF) as usize
    }

    /// Offset within the final 4 KiB page — bits 11:0.
    pub const fn page_offset(self) -> u64 {
        self.0 & 0xFFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_virtual_base_decomposes_to_pml4_511_pdpt_510() {
        // 0xFFFFFFFF80000000 — KERNEL_VIRTUAL_BASE per ADR-002/ADR-005
        // — is the same address boot/paging.c's own derivation
        // (documented in ADR-005) computed by hand. Cross-checking
        // that derivation against this code's arithmetic here is
        // exactly the kind of "don't just trust the comment" test
        // this project's coding standards ask for.
        let addr = VirtAddr::new(0xFFFF_FFFF_8000_0000);
        assert_eq!(addr.pml4_index(), 511);
        assert_eq!(addr.pdpt_index(), 510);
        assert_eq!(addr.pd_index(), 0);
        assert_eq!(addr.pt_index(), 0);
        assert_eq!(addr.page_offset(), 0);
    }

    #[test]
    fn identity_map_base_decomposes_to_pml4_0() {
        let addr = VirtAddr::new(0x0000_0000_0000_0000);
        assert_eq!(addr.pml4_index(), 0);
        assert_eq!(addr.pdpt_index(), 0);
        assert_eq!(addr.pd_index(), 0);
        assert_eq!(addr.pt_index(), 0);
    }

    #[test]
    fn kernel_heap_base_decomposes_correctly() {
        // 0xFFFF880000000000 — the kernel heap region's virtual base
        // per ADR-002 — must land in a DIFFERENT PML4 entry than the
        // kernel image (511) and the identity map (0), since the VMM
        // must be able to create mappings there without disturbing
        // either existing region ("higher-half kernel mapping
        // compatibility").
        let addr = VirtAddr::new(0xFFFF_8800_0000_0000);
        assert_ne!(addr.pml4_index(), 511);
        assert_ne!(addr.pml4_index(), 0);
    }

    #[test]
    fn indices_and_offset_recombine_to_the_original_address() {
        let original = 0xFFFF_8123_4567_8ABC_u64 & !0xFFF; // page-align it
        let addr = VirtAddr::new(original);
        let recombined = ((addr.pml4_index() as u64) << 39)
            | ((addr.pdpt_index() as u64) << 30)
            | ((addr.pd_index() as u64) << 21)
            | ((addr.pt_index() as u64) << 12)
            | addr.page_offset();
        // Sign-extend bit 47 into bits 63:48 for canonical form, the
        // same way a real address in the high canonical half already
        // is — this test only checks the low 48 bits round-trip,
        // which is all this decomposition claims to preserve.
        assert_eq!(recombined & 0xFFFF_FFFF_FFFF, original & 0xFFFF_FFFF_FFFF);
    }

    #[test]
    fn page_alignment_check() {
        assert!(VirtAddr::new(0x1000).is_page_aligned());
        assert!(VirtAddr::new(0).is_page_aligned());
        assert!(!VirtAddr::new(0x1001).is_page_aligned());
    }
}
