//! phys_addr.rs — a newtype for physical addresses.
//!
//! A bare `u64` says nothing about whether it's a physical or virtual
//! address — a class of bug (passing one where the other is expected)
//! that becomes very easy to introduce once the virtual memory manager
//! exists alongside this physical allocator. Wrapping both in distinct
//! types now, while there's only one of them, means the compiler
//! catches that mistake everywhere from here on, per
//! `docs/kernel/CODING_STANDARDS.md`'s guidance on pushing safety
//! guarantees into the type system at API boundaries.

/// The frame size this entire memory manager subsystem works in.
/// 4 KiB, per the x86_64 architecture's base page size — not a
/// design choice, a hardware fact.
pub const FRAME_SIZE: u64 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysAddr(u64);

impl PhysAddr {
    /// Construct from a raw value. Not validated here — callers that
    /// need "is this a real, in-range physical address" should ask
    /// the frame allocator, which is the component that actually knows
    /// what's real; this type is a labeling tool, not a validator.
    pub const fn new(addr: u64) -> Self {
        PhysAddr(addr)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// The physical frame number this address falls in — the address
    /// divided by `FRAME_SIZE`, truncating any in-frame offset. Used
    /// throughout the frame allocator to convert between addresses
    /// and bitmap bit indices.
    pub const fn frame_number(self) -> usize {
        (self.0 / FRAME_SIZE) as usize
    }

    /// Reconstruct the frame-aligned address for a given frame number
    /// — the inverse of `frame_number`.
    pub const fn from_frame_number(frame: usize) -> Self {
        PhysAddr(frame as u64 * FRAME_SIZE)
    }

    /// True if this address is exactly frame-aligned. The frame
    /// allocator's `init` uses this to validate memory map entries
    /// per requirement 3 ("validate all memory regions before use") —
    /// a `PhysicalStart` that isn't frame-aligned indicates either a
    /// firmware bug or a corrupted `BootInfo`, and is rejected rather
    /// than silently rounded.
    pub const fn is_frame_aligned(self) -> bool {
        self.0 % FRAME_SIZE == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_number_round_trips() {
        let addr = PhysAddr::new(0x123_000);
        assert_eq!(addr.frame_number(), 0x123);
        assert_eq!(PhysAddr::from_frame_number(0x123), addr);
    }

    #[test]
    fn frame_number_truncates_in_frame_offset() {
        // An address that isn't frame-aligned still maps to the frame
        // that contains it — this is intentional (frame_number is a
        // pure conversion, not a validator; is_frame_aligned is the
        // validator), but the behavior is worth pinning down in a
        // test so a future refactor can't silently change it.
        let addr = PhysAddr::new(0x123_045);
        assert_eq!(addr.frame_number(), 0x123);
    }

    #[test]
    fn alignment_check() {
        assert!(PhysAddr::new(0x1000).is_frame_aligned());
        assert!(PhysAddr::new(0).is_frame_aligned());
        assert!(!PhysAddr::new(0x1001).is_frame_aligned());
        assert!(!PhysAddr::new(1).is_frame_aligned());
    }
}
