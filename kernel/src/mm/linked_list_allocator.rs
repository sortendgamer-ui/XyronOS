//! linked_list_allocator.rs — pure free-list allocation logic.
//!
//! Deliberately has no dependency on the VMM, frame allocator, or any
//! real hardware — it operates entirely on a `[start, start+size)`
//! byte range handed to it by the caller, which can just as easily be
//! a `Vec<u8>`-backed buffer in a host unit test as real mapped kernel
//! memory. See `docs/kernel/MEMORY_MANAGER_DESIGN.md`'s "Concrete
//! decisions" section for the full design reasoning behind every
//! choice below (address-sorted free list with coalescing, minimum
//! block size, gap handling).

use core::alloc::Layout;
use core::mem::{align_of, size_of};

/// One node in the free list. Lives INSIDE the free memory it
/// describes — a freed block's own first bytes store this struct, so
/// there is no separate metadata array to keep in sync. `next` is a
/// raw pointer (not a safe reference) deliberately: the coalescing
/// logic in `add_free_region` needs to relocate/merge/remove nodes in
/// ways that don't have a natural non-`unsafe` expression with
/// lifetime-checked references, and raw pointers make every one of
/// those operations a plain, auditable pointer write instead of
/// fighting the borrow checker over operations that are sound by
/// construction (every pointer here always refers to memory this
/// allocator was given ownership of via `init`/`add_free_region`).
#[repr(C)]
struct FreeListNode {
    size: usize,
    next: *mut FreeListNode,
}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

/// Rounds a `Layout` up to the minimum this allocator can track: at
/// least `size_of::<FreeListNode>()` bytes (a freed block must be able
/// to hold its own header) and at least `align_of::<FreeListNode>()`
/// alignment. Used identically by both `alloc` and `dealloc` — see
/// the design doc for why deriving this the same way on both sides,
/// rather than storing it anywhere extra, is both sufficient and
/// exactly what `GlobalAlloc`'s API shape already assumes.
fn adjusted_size_align(layout: Layout) -> (usize, usize) {
    let layout = layout
        .align_to(align_of::<FreeListNode>())
        .expect("heap allocation layout alignment overflowed")
        .pad_to_align();
    let size = layout.size().max(size_of::<FreeListNode>());
    (size, layout.align())
}

pub struct LinkedListAllocator {
    /// Sentinel head node: `size` is always 0 and never itself
    /// represents real free memory — it exists purely so every other
    /// operation (`add_free_region`, `alloc_first_fit`) can treat "insert
    /// before the first real node" the same as "insert between two real
    /// nodes," with no special-cased empty-list branch.
    head: FreeListNode,
}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        LinkedListAllocator {
            head: FreeListNode {
                size: 0,
                next: core::ptr::null_mut(),
            },
        }
    }

    /// Add `[addr, addr + size)` to the free list, coalescing with an
    /// immediately-adjacent existing free block on either side if one
    /// exists. Keeps the list sorted by address — see the design doc
    /// for why: coalescing by direct address comparison (no boundary
    /// tags needed) only works if the list is walked in address order.
    ///
    /// This is the sole entry point for giving this allocator memory
    /// to manage — called for the very first region exactly the same
    /// way as for every later growth, with no separate "init" case:
    /// on an empty list, every free/insert branch below naturally
    /// degrades to "insert as the only node," so no special-casing is
    /// needed.
    ///
    /// # Safety
    /// `[addr, addr + size)` must be a region this allocator has
    /// exclusive, valid-for-writes ownership of for as long as the
    /// allocator exists — real mapped memory in the real kernel, or a
    /// `Vec<u8>`'s backing storage in a test. Must not overlap any
    /// region already added via a prior call. `addr` must be aligned
    /// to `align_of::<FreeListNode>()` and `size` must be at least
    /// `size_of::<FreeListNode>()` (both always true for every call
    /// site in this file, which only ever calls this with regions
    /// already validated by `adjusted_size_align` or by the heap's own
    /// page-granularity growth).
    pub unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        assert_eq!(
            addr % align_of::<FreeListNode>(),
            0,
            "free region address is not aligned for FreeListNode"
        );
        assert!(
            size >= size_of::<FreeListNode>(),
            "free region is smaller than one FreeListNode"
        );

        let head_ptr: *mut FreeListNode = &mut self.head;

        // Walk to the last node whose address is < addr, keeping the
        // list sorted — `prev` ends up either the sentinel head (if
        // addr is the lowest free address so far) or a real node.
        let mut prev: *mut FreeListNode = head_ptr;
        while !(*prev).next.is_null() && ((*prev).next as usize) < addr {
            prev = (*prev).next;
        }
        let next: *mut FreeListNode = (*prev).next;

        let prev_is_real = prev != head_ptr;
        let merge_backward = prev_is_real && (prev as usize) + (*prev).size == addr;
        let merge_forward = !next.is_null() && addr + size == next as usize;

        match (merge_backward, merge_forward) {
            (true, true) => {
                // The new region exactly bridges prev and next — one
                // block now spans all three; next is removed entirely.
                (*prev).size += size + (*next).size;
                (*prev).next = (*next).next;
            }
            (true, false) => {
                // Extends prev; prev's own `next` link is unchanged.
                (*prev).size += size;
            }
            (false, true) => {
                // The new region immediately precedes `next`. The
                // combined block must be written starting at `addr`
                // (the earlier address) — `next`'s old header position
                // is being absorbed, not kept, so a fresh node is
                // written at `addr` and spliced in where `next` was.
                let merged = addr as *mut FreeListNode;
                (*merged).size = size + (*next).size;
                (*merged).next = (*next).next;
                (*prev).next = merged;
            }
            (false, false) => {
                let node = addr as *mut FreeListNode;
                (*node).size = size;
                (*node).next = next;
                (*prev).next = node;
            }
        }
    }

    /// First-fit search: find the first free block able to hold
    /// `layout` (after alignment/minimum-size adjustment), remove it
    /// from the free list, and return a pointer to it — splitting off
    /// and re-adding any leftover front/back space that's large enough
    /// to track (see the design doc's gap-handling decision). Returns
    /// null if no block is large enough anywhere in the list.
    ///
    /// # Safety
    /// The allocator must have been `init`-ed (directly, or indirectly
    /// via `add_free_region`) with at least one real region before
    /// this can return anything but null.
    pub unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let (size, align) = adjusted_size_align(layout);

        let head_ptr: *mut FreeListNode = &mut self.head;
        let mut prev: *mut FreeListNode = head_ptr;
        let mut current: *mut FreeListNode = (*head_ptr).next;

        while !current.is_null() {
            let region_start = current as usize;
            let region_size = (*current).size;
            let region_end = region_start + region_size;
            let alloc_start = align_up(region_start, align);

            if let Some(alloc_end) = alloc_start.checked_add(size) {
                if alloc_end <= region_end {
                    // Found a fit — unlink `current` from the list
                    // first, then re-add whichever leftover edges are
                    // large enough to track as their own free regions.
                    let next = (*current).next;
                    (*prev).next = next;

                    let front_gap = alloc_start - region_start;
                    if front_gap >= size_of::<FreeListNode>() {
                        self.add_free_region(region_start, front_gap);
                    }
                    // else: lost — see design doc's gap-handling note.

                    let back_gap = region_end - alloc_end;
                    if back_gap >= size_of::<FreeListNode>() {
                        self.add_free_region(alloc_end, back_gap);
                    }
                    // else: lost — same reasoning, symmetric with the
                    // front gap.

                    return alloc_start as *mut u8;
                }
            }

            prev = current;
            current = (*current).next;
        }

        core::ptr::null_mut()
    }

    /// Return a previously-`alloc`'d block to the free list. `layout`
    /// must be the exact same `Layout` passed to the `alloc` call that
    /// returned `ptr` — `GlobalAlloc`'s contract already guarantees
    /// this, and `adjusted_size_align` depends on it to reconstruct
    /// the same size `alloc` reserved without needing it stored
    /// anywhere separately.
    ///
    /// # Safety
    /// `ptr` must have been returned by a prior `alloc(layout)` call
    /// on this same allocator instance, and not already freed.
    pub unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let (size, _align) = adjusted_size_align(layout);
        self.add_free_region(ptr as usize, size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Layout as StdLayout;

    /// Host-test helper: leaks a `Vec<u8>` (host heap memory) to get a
    /// stable, never-moved backing buffer for a `LinkedListAllocator`
    /// to manage — playing exactly the role real mapped kernel memory
    /// plays in the boot self-test, without needing real hardware.
    fn test_heap(size: usize) -> (LinkedListAllocator, usize) {
        let buf: &'static mut [u8] = Box::leak(vec![0u8; size].into_boxed_slice());
        let start = buf.as_mut_ptr() as usize;
        let mut alloc = LinkedListAllocator::new();
        unsafe {
            alloc.add_free_region(start, size);
        }
        (alloc, start)
    }

    #[test]
    fn single_allocation_succeeds_and_is_writable() {
        let (mut alloc, _start) = test_heap(4096);
        let layout = StdLayout::from_size_align(64, 8).unwrap();
        let ptr = unsafe { alloc.alloc(layout) };
        assert!(!ptr.is_null());
        unsafe {
            core::ptr::write_bytes(ptr, 0xAB, 64);
            for i in 0..64 {
                assert_eq!(*ptr.add(i), 0xAB);
            }
        }
    }

    #[test]
    fn allocations_never_overlap() {
        let (mut alloc, _start) = test_heap(4096);
        let layout = StdLayout::from_size_align(64, 8).unwrap();
        let mut ptrs = std::vec::Vec::new();
        for _ in 0..10 {
            let p = unsafe { alloc.alloc(layout) };
            assert!(!p.is_null());
            ptrs.push(p as usize);
        }
        for i in 0..ptrs.len() {
            for j in (i + 1)..ptrs.len() {
                let (a, b) = (ptrs[i], ptrs[j]);
                let overlap = a < b + 64 && b < a + 64;
                assert!(!overlap, "allocations {a:#x} and {b:#x} overlap");
            }
        }
    }

    #[test]
    fn alloc_returns_null_when_heap_is_exhausted() {
        let (mut alloc, _start) = test_heap(256);
        let layout = StdLayout::from_size_align(256, 8).unwrap();
        // First allocation of the entire heap should succeed...
        let p1 = unsafe { alloc.alloc(layout) };
        assert!(!p1.is_null());
        // ...a second, with nothing freed in between, must fail.
        let p2 = unsafe { alloc.alloc(StdLayout::from_size_align(1, 1).unwrap()) };
        assert!(p2.is_null());
    }

    #[test]
    fn freed_memory_becomes_reusable() {
        let (mut alloc, _start) = test_heap(256);
        let layout = StdLayout::from_size_align(256, 8).unwrap();
        let p1 = unsafe { alloc.alloc(layout) };
        assert!(!p1.is_null());
        unsafe {
            alloc.dealloc(p1, layout);
        }
        let p2 = unsafe { alloc.alloc(layout) };
        assert!(!p2.is_null(), "freed memory should have been reusable");
    }

    #[test]
    fn adjacent_freed_blocks_coalesce_into_one_reusable_region() {
        // This is the property a non-coalescing free list could never
        // provide: two separately-freed halves must combine into
        // something big enough to satisfy an allocation neither half
        // alone could — see the design doc's coalescing rationale.
        let (mut alloc, _start) = test_heap(256);
        let half = StdLayout::from_size_align(128, 8).unwrap();
        let whole = StdLayout::from_size_align(256, 8).unwrap();

        let a = unsafe { alloc.alloc(half) };
        let b = unsafe { alloc.alloc(half) };
        assert!(!a.is_null() && !b.is_null());
        // Heap is now fully allocated as two adjacent 128-byte blocks.
        assert!(unsafe { alloc.alloc(StdLayout::from_size_align(1, 1).unwrap()) }.is_null());

        unsafe {
            alloc.dealloc(a, half);
            alloc.dealloc(b, half);
        }

        let reunited = unsafe { alloc.alloc(whole) };
        assert!(
            !reunited.is_null(),
            "freeing two adjacent blocks should coalesce into one 256-byte region"
        );
    }

    #[test]
    fn coalescing_works_regardless_of_free_order() {
        // Free list is address-sorted with coalescing checked against
        // both neighbors on every insert — this must hold whether the
        // lower or higher address is freed first.
        let (mut alloc, _start) = test_heap(256);
        let half = StdLayout::from_size_align(128, 8).unwrap();
        let whole = StdLayout::from_size_align(256, 8).unwrap();

        let a = unsafe { alloc.alloc(half) };
        let b = unsafe { alloc.alloc(half) };
        unsafe {
            // Free the SECOND (higher-address) block first this time.
            alloc.dealloc(b, half);
            alloc.dealloc(a, half);
        }
        let reunited = unsafe { alloc.alloc(whole) };
        assert!(!reunited.is_null());
    }

    #[test]
    fn three_adjacent_frees_coalesce_into_one_block() {
        let (mut alloc, _start) = test_heap(192);
        let third = StdLayout::from_size_align(64, 8).unwrap();
        let whole = StdLayout::from_size_align(192, 8).unwrap();

        let a = unsafe { alloc.alloc(third) };
        let b = unsafe { alloc.alloc(third) };
        let c = unsafe { alloc.alloc(third) };
        assert!(!a.is_null() && !b.is_null() && !c.is_null());

        unsafe {
            // Free the middle block first, then both outer ones — by
            // the time the outer blocks are freed, the middle one
            // must already be sitting in the list ready to merge with
            // each in turn (forward merge with `a`, backward merge
            // extended further by `c`).
            alloc.dealloc(b, third);
            alloc.dealloc(a, third);
            alloc.dealloc(c, third);
        }

        let reunited = unsafe { alloc.alloc(whole) };
        assert!(
            !reunited.is_null(),
            "three adjacent frees, in this order, should still fully coalesce"
        );
    }

    #[test]
    fn respects_alignment_greater_than_default() {
        let (mut alloc, start) = test_heap(4096);
        let layout = StdLayout::from_size_align(32, 64).unwrap();
        let ptr = unsafe { alloc.alloc(layout) };
        assert!(!ptr.is_null());
        assert_eq!(
            (ptr as usize) % 64,
            0,
            "returned pointer does not satisfy the requested 64-byte alignment"
        );
        let _ = start; // only used to keep the backing buffer's origin in scope for clarity
    }

    #[test]
    fn zero_sized_and_tiny_allocations_still_return_a_valid_distinct_pointer() {
        // A 1-byte, 1-align request must still be rounded up to at
        // least size_of::<FreeListNode>() internally (see
        // adjusted_size_align) — this test checks the observable
        // contract (a valid, usable pointer), not the internal
        // rounding directly.
        let (mut alloc, _start) = test_heap(4096);
        let layout = StdLayout::from_size_align(1, 1).unwrap();
        let p1 = unsafe { alloc.alloc(layout) };
        let p2 = unsafe { alloc.alloc(layout) };
        assert!(!p1.is_null() && !p2.is_null());
        assert_ne!(p1, p2);
    }

    #[test]
    fn add_free_region_rejects_misaligned_address() {
        let mut alloc = LinkedListAllocator::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            alloc.add_free_region(1, 4096);
        }));
        assert!(result.is_err(), "misaligned free region should panic, not silently corrupt state");
    }

    #[test]
    fn add_free_region_rejects_undersized_region() {
        let mut alloc = LinkedListAllocator::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            alloc.add_free_region(align_of::<FreeListNode>(), 1);
        }));
        assert!(result.is_err(), "undersized free region should panic, not silently corrupt state");
    }
}
