//! frame_allocator.rs — bitmap physical frame allocator.
//!
//! See `docs/kernel/MEMORY_MANAGER_DESIGN.md` for the full design
//! rationale (why a bitmap, why the bitmap's storage is constrained
//! below the identity-map limit, why a bitmap over a free-list). This
//! file is the implementation of that design.

use crate::boot_info::BootInfo;
use crate::mm::memory_map::{MemoryMapIter, EFI_CONVENTIONAL_MEMORY};
use crate::mm::phys_addr::{PhysAddr, FRAME_SIZE};

/// Physical addresses at or above this are not covered by the page
/// tables the bootloader built — ADR-005's identity map covers only
/// the first 4 GiB. Until the virtual memory manager subsystem exists
/// (next kernel subsystem, not yet built), the kernel cannot
/// dereference any physical address at or above this value. Must
/// match `boot/include/boot_defs.h`'s `IDENTITY_MAP_LIMIT` exactly;
/// kept in sync manually, the same situation as `boot_info.rs`
/// mirroring `boot_info.h` — no shared build step exists between the
/// bootloader and kernel to enforce this automatically.
pub(crate) const IDENTITY_MAP_LIMIT: u64 = 0x1_0000_0000;

/// Sanity bound on bitmap size, covering roughly 2 TiB of tracked RAM
/// at 1 bit per 4 KiB frame. Real hardware and QEMU test
/// configurations are far below this; a memory map implying more than
/// this is treated as corrupt `BootInfo` rather than trusted, per
/// requirement 3 ("validate all memory regions before use") — this is
/// the whole-map-level validation to go with the per-entry validation
/// below.
const MAX_BITMAP_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitError {
    /// `memory_map_size_bytes` is smaller than
    /// `entry_count * descriptor_size` — the map cannot possibly
    /// contain the entries it claims to.
    InconsistentMemoryMap,
    /// No `EfiConventionalMemory` region, entirely below
    /// `IDENTITY_MAP_LIMIT`, was large enough to hold the bitmap.
    NoSuitableBitmapRegion,
    /// The memory map implies a physical address space larger than
    /// `MAX_BITMAP_BYTES` can track — see that constant's doc comment.
    ImpliedAddressSpaceTooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeallocError {
    /// The given address's frame number is beyond every frame this
    /// allocator was initialized to track.
    OutOfRange,
    /// The frame was already free — deallocating it again would be a
    /// double-free. Returned as an error (per
    /// `docs/kernel/CODING_STANDARDS.md`'s panic-vs-Result guidance:
    /// a caller can reasonably decide what to do about this) rather
    /// than panicking the whole kernel.
    DoubleFree,
}

/// A bitmap-based physical frame allocator: one bit per 4 KiB frame,
/// `0` = free, `1` = used.
pub struct FrameAllocator {
    bitmap: &'static mut [u8],
    total_frames: usize,
    frames_free: usize,
    /// Where the next `allocate()` scan starts — a simple heuristic
    /// (not required for correctness) so repeated allocations after
    /// the low frames fill up don't all re-scan from frame zero every
    /// time. Per the design doc, allocation itself stays O(n)
    /// worst-case; this only improves the common case.
    next_hint: usize,
}

impl FrameAllocator {
    /// Build a frame allocator from the UEFI memory map in
    /// `boot_info`.
    ///
    /// # Safety
    /// - `boot_info` must already be validated (`boot_info.is_valid()`
    ///   checked by the caller) and must be the real handoff struct
    ///   the bootloader populated.
    /// - Must be called at most once. This claims a region of physical
    ///   memory as the bitmap's permanent backing storage; calling it
    ///   again would either overwrite that claim or place a second
    ///   bitmap over memory already in use.
    /// - The page tables the bootloader built (identity map of the
    ///   first 4 GiB, ADR-005) must still be the active page tables —
    ///   this function writes directly to a physical address through
    ///   that identity mapping.
    pub unsafe fn init(boot_info: &BootInfo) -> Result<Self, InitError> {
        // ---- Upfront whole-map validation (requirement 3) ----------
        // Checked once, before any entry is trusted, rather than
        // relied upon implicitly by every later pass over the map.
        let entry_count = boot_info.memory_map_entry_count as usize;
        let descriptor_size = boot_info.memory_map_descriptor_size as usize;
        let claimed_size = entry_count
            .checked_mul(descriptor_size)
            .ok_or(InitError::InconsistentMemoryMap)?;
        if (boot_info.memory_map_size_bytes as usize) < claimed_size {
            return Err(InitError::InconsistentMemoryMap);
        }

        // ---- Pass 1: find the highest address any entry describes,
        // to size the bitmap. Every entry is validated the same way
        // every subsequent pass validates entries (requirement 3):
        // skip a zero-length region, skip a non-frame-aligned start
        // (the UEFI spec guarantees frame alignment; a violation means
        // something is wrong and this entry cannot be trusted), skip
        // an entry whose end address would overflow u64.
        let mut highest_addr: u64 = 0;
        // SAFETY: boot_info is the real, validated handoff struct per
        // this function's own safety contract, which is exactly what
        // MemoryMapIter::new requires of its caller.
        for entry in unsafe { MemoryMapIter::new(boot_info) } {
            if entry.number_of_pages == 0 {
                continue;
            }
            if !entry.physical_start.is_frame_aligned() {
                continue;
            }
            let Some(end) = entry.end_addr() else {
                continue;
            };
            if end > highest_addr {
                highest_addr = end;
            }
        }

        if highest_addr == 0 {
            // No valid entry at all — an empty or entirely-corrupt map.
            return Err(InitError::InconsistentMemoryMap);
        }

        let total_frames = (highest_addr / FRAME_SIZE) as usize;
        let bitmap_bytes = total_frames.div_ceil(8);
        if bitmap_bytes == 0 || bitmap_bytes > MAX_BITMAP_BYTES {
            return Err(InitError::ImpliedAddressSpaceTooLarge);
        }

        // ---- Pass 2: find a free, entirely-below-the-identity-map-
        // limit, large-enough region to host the bitmap itself. First
        // fit, not best fit — see design doc for why that's the
        // deliberate choice here.
        let mut bitmap_phys_addr: Option<u64> = None;
        // SAFETY: same as Pass 1.
        for entry in unsafe { MemoryMapIter::new(boot_info) } {
            if entry.typ != EFI_CONVENTIONAL_MEMORY {
                continue;
            }
            if entry.number_of_pages == 0 {
                continue;
            }
            if !entry.physical_start.is_frame_aligned() {
                continue;
            }
            let Some(end) = entry.end_addr() else {
                continue;
            };
            if end > IDENTITY_MAP_LIMIT {
                // This region (or the tail of it) is not directly
                // dereferenceable yet — only accept regions entirely
                // below the limit, not merely the below-limit portion
                // of a partially-covered one, to keep this already
                // subtle piece of logic simple and obviously correct.
                continue;
            }
            let region_size = end - entry.physical_start.as_u64();
            if region_size >= bitmap_bytes as u64 {
                bitmap_phys_addr = Some(entry.physical_start.as_u64());
                break;
            }
        }

        let bitmap_phys_addr = bitmap_phys_addr.ok_or(InitError::NoSuitableBitmapRegion)?;

        // SAFETY: bitmap_phys_addr was just verified to fall inside a
        // region entirely below IDENTITY_MAP_LIMIT, so it is
        // dereferenceable as a virtual address equal to its physical
        // value (the bootloader's identity map, ADR-005). bitmap_bytes
        // was verified <= the source region's own size immediately
        // above, so the full slice stays within memory that region
        // actually covers. This function's own safety contract
        // guarantees this runs at most once, so no other code holds a
        // conflicting reference to this memory.
        let bitmap: &'static mut [u8] =
            unsafe { core::slice::from_raw_parts_mut(bitmap_phys_addr as *mut u8, bitmap_bytes) };

        // Default every frame to USED. This is the safe default
        // direction: an entry this code fails to recognize or
        // classify simply stays marked used forever, which can only
        // make the allocator overly conservative, never unsound.
        for b in bitmap.iter_mut() {
            *b = 0xFF;
        }

        let mut allocator = FrameAllocator {
            bitmap,
            total_frames,
            frames_free: 0,
            next_hint: 0,
        };

        // ---- Pass 3: mark every validated Conventional frame free.
        // SAFETY: same as Pass 1.
        for entry in unsafe { MemoryMapIter::new(boot_info) } {
            if entry.typ != EFI_CONVENTIONAL_MEMORY {
                continue;
            }
            if entry.number_of_pages == 0 {
                continue;
            }
            if !entry.physical_start.is_frame_aligned() {
                continue;
            }
            if entry.end_addr().is_none() {
                continue;
            }

            let start_frame = entry.physical_start.frame_number();
            let frame_count = entry.number_of_pages as usize;
            for frame in start_frame..start_frame.saturating_add(frame_count) {
                if frame >= allocator.total_frames {
                    break;
                }
                allocator.mark_free(frame);
            }
        }

        // The bitmap's own frames came from a Conventional region and
        // were just marked free by Pass 3 — reserve them again now
        // that their exact frame range is known, so nothing can ever
        // be allocated on top of the allocator's own bookkeeping.
        let bitmap_start_frame = PhysAddr::new(bitmap_phys_addr).frame_number();
        let bitmap_frame_count = (bitmap_bytes as u64).div_ceil(FRAME_SIZE) as usize;
        for frame in bitmap_start_frame..bitmap_start_frame.saturating_add(bitmap_frame_count) {
            if frame < allocator.total_frames {
                allocator.mark_used(frame);
            }
        }

        Ok(allocator)
    }

    fn is_free(&self, frame: usize) -> bool {
        (self.bitmap[frame / 8] & (1 << (frame % 8))) == 0
    }

    fn mark_used(&mut self, frame: usize) {
        if self.is_free(frame) {
            self.frames_free -= 1;
        }
        self.bitmap[frame / 8] |= 1 << (frame % 8);
    }

    fn mark_free(&mut self, frame: usize) {
        if !self.is_free(frame) {
            self.frames_free += 1;
        }
        self.bitmap[frame / 8] &= !(1 << (frame % 8));
    }

    /// Allocate one free frame, or `None` if every tracked frame is
    /// used. O(n) worst case (see design doc) — scans from
    /// `next_hint`, wrapping around, so repeated calls don't all
    /// restart from frame zero once low memory fills up.
    pub fn allocate(&mut self) -> Option<PhysAddr> {
        let total = self.total_frames;
        if total == 0 {
            return None;
        }
        for offset in 0..total {
            let frame = (self.next_hint + offset) % total;
            if self.is_free(frame) {
                self.mark_used(frame);
                self.next_hint = (frame + 1) % total;
                return Some(PhysAddr::from_frame_number(frame));
            }
        }
        None
    }

    /// Free a previously allocated frame.
    pub fn deallocate(&mut self, addr: PhysAddr) -> Result<(), DeallocError> {
        let frame = addr.frame_number();
        if frame >= self.total_frames {
            return Err(DeallocError::OutOfRange);
        }
        if self.is_free(frame) {
            return Err(DeallocError::DoubleFree);
        }
        self.mark_free(frame);
        Ok(())
    }

    /// Allocate one free frame whose address is strictly below `limit`.
    ///
    /// Needed by the virtual memory manager (`mm/vmm.rs`): a new
    /// page-table page must be a physical address the kernel can
    /// directly write through the identity map to initialize (zero it,
    /// then write entries into it) — `allocate()` alone offers no such
    /// guarantee, since this allocator tracks the *entire* physical
    /// memory map, including frames above `IDENTITY_MAP_LIMIT` that
    /// are valid to allocate (bookkeeping-wise) but not yet directly
    /// dereferenceable by any code that doesn't have another path to
    /// them — see `docs/kernel/MEMORY_MANAGER_DESIGN.md`.
    ///
    /// Same O(n) worst-case scan as `allocate()`, over the restricted
    /// prefix of frames below `limit` only.
    pub fn allocate_below(&mut self, limit: PhysAddr) -> Option<PhysAddr> {
        let bound = core::cmp::min(self.total_frames, limit.frame_number());
        for frame in 0..bound {
            if self.is_free(frame) {
                self.mark_used(frame);
                return Some(PhysAddr::from_frame_number(frame));
            }
        }
        None
    }

    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    pub fn frames_free(&self) -> usize {
        self.frames_free
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the pure bitmap logic, run on the HOST target
    //! (`cargo test --target <host-triple>`, overriding the custom
    //! bare-metal default in `.cargo/config.toml`) — see
    //! `kernel/README.md`. These construct a `FrameAllocator` directly
    //! from an in-memory bitmap (bypassing `init`'s UEFI memory map
    //! parsing, which is exercised instead by the boot-time
    //! integration self-test in `main.rs`, per requirement 7) to keep
    //! this file's unit tests focused purely on allocate/free/
    //! double-free bookkeeping correctness.
    use super::*;

    fn test_allocator(total_frames: usize) -> FrameAllocator {
        let bitmap_bytes = total_frames.div_ceil(8);
        let bitmap: &'static mut [u8] = Box::leak(vec![0u8; bitmap_bytes].into_boxed_slice());
        FrameAllocator {
            bitmap,
            total_frames,
            frames_free: total_frames,
            next_hint: 0,
        }
    }

    #[test]
    fn allocate_returns_distinct_frames() {
        let mut alloc = test_allocator(8);
        let a = alloc.allocate().unwrap();
        let b = alloc.allocate().unwrap();
        assert_ne!(a, b);
        assert_eq!(alloc.frames_free(), 6);
    }

    #[test]
    fn allocate_exhausts_and_returns_none() {
        let mut alloc = test_allocator(4);
        for _ in 0..4 {
            assert!(alloc.allocate().is_some());
        }
        assert_eq!(alloc.allocate(), None);
        assert_eq!(alloc.frames_free(), 0);
    }

    #[test]
    fn deallocate_then_reallocate_reuses_frame() {
        let mut alloc = test_allocator(2);
        let a = alloc.allocate().unwrap();
        let _b = alloc.allocate().unwrap();
        assert_eq!(alloc.allocate(), None);

        alloc.deallocate(a).unwrap();
        assert_eq!(alloc.frames_free(), 1);

        let reused = alloc.allocate().unwrap();
        assert_eq!(reused, a);
    }

    #[test]
    fn double_free_is_rejected() {
        let mut alloc = test_allocator(4);
        let a = alloc.allocate().unwrap();
        alloc.deallocate(a).unwrap();
        assert_eq!(alloc.deallocate(a), Err(DeallocError::DoubleFree));
    }

    #[test]
    fn deallocate_out_of_range_is_rejected() {
        let mut alloc = test_allocator(4);
        let bogus = PhysAddr::from_frame_number(1000);
        assert_eq!(alloc.deallocate(bogus), Err(DeallocError::OutOfRange));
    }

    #[test]
    fn allocation_never_returns_a_frame_still_marked_used() {
        // Regression check for the mark_used/mark_free bookkeeping:
        // pre-mark half the frames used (as init's "default to used,
        // free the validated Conventional ones" pattern does), then
        // confirm allocate() only ever returns frames from the free
        // half.
        let mut alloc = test_allocator(8);
        for frame in 0..4 {
            alloc.mark_used(frame);
        }
        alloc.frames_free = 4;

        let mut seen = std::collections::HashSet::new();
        for _ in 0..4 {
            let addr = alloc.allocate().expect("should have 4 free frames");
            assert!(addr.frame_number() >= 4, "allocated a pre-used frame");
            seen.insert(addr.frame_number());
        }
        assert_eq!(seen.len(), 4);
        assert_eq!(alloc.allocate(), None);
    }

    #[test]
    fn allocate_below_only_returns_frames_under_the_limit() {
        let mut alloc = test_allocator(16);
        // limit.frame_number() == 8: frames 0..8 are eligible,
        // frames 8..16 are not, regardless of free/used state.
        let limit = PhysAddr::from_frame_number(8);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..8 {
            let addr = alloc
                .allocate_below(limit)
                .expect("8 frames should be available below the limit");
            assert!(
                addr.frame_number() < 8,
                "allocate_below returned a frame at or above its limit"
            );
            seen.insert(addr.frame_number());
        }
        assert_eq!(seen.len(), 8, "allocate_below returned a frame twice");
        // Every eligible frame is now used; a 9th call must fail even
        // though frames 8..16 are still entirely free — allocate_below
        // must never fall back to allocating above the limit.
        assert_eq!(alloc.allocate_below(limit), None);
    }

    #[test]
    fn allocate_below_respects_frames_already_used_by_allocate() {
        let mut alloc = test_allocator(4);
        let limit = PhysAddr::from_frame_number(4);
        let taken = alloc.allocate().unwrap(); // may take any frame 0..4
        let below = alloc
            .allocate_below(limit)
            .expect("3 frames should remain");
        assert_ne!(
            taken, below,
            "allocate_below returned a frame allocate() already gave out"
        );
    }
}
