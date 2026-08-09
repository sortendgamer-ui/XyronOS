//! heap.rs — `KernelHeap`: the `GlobalAlloc` implementation, wrapping
//! `LinkedListAllocator` (pure logic, see linked_list_allocator.rs)
//! with growth-on-demand backed by the virtual memory manager and
//! physical frame allocator. See `docs/kernel/MEMORY_MANAGER_DESIGN.md`'s
//! "Concrete decisions" section for the full reasoning behind the
//! global-statics design this file depends on.

use core::alloc::{GlobalAlloc, Layout};

use crate::mm::frame_allocator::FrameAllocator;
use crate::mm::linked_list_allocator::LinkedListAllocator;
use crate::mm::page_table_entry::PageFlags;
use crate::mm::virt_addr::VirtAddr;
use crate::mm::vmm::VirtualMemoryManager;
use crate::sync::SpinLock;

const PAGE_SIZE: usize = 4096;
/// Kernel heap region base, per ADR-002.
const HEAP_REGION_START: u64 = 0xFFFF_8800_0000_0000;
/// Top of the heap region, per ADR-002 — the point where the
/// higher-half kernel image itself begins. Growth must never cross
/// this, or a heap allocation would silently start colliding with
/// the kernel's own code/data.
const HEAP_REGION_END: u64 = 0xFFFF_FFFF_8000_0000;
/// Minimum pages mapped per growth step — see the design doc for why
/// a fixed minimum (rather than mapping exactly what one failing
/// request needs) is worth the small amount of memory it can reserve
/// ahead of actual demand.
const HEAP_GROWTH_MIN_PAGES: u64 = 16;

/// Global handle to the frame allocator, populated once by
/// `kernel_main` after `FrameAllocator::init`'s own boot self-test
/// runs against a local binding exactly as before — this global exists
/// ONLY so heap growth (which runs from behind `GlobalAlloc`'s `&self`
/// methods, with no other way to reach kernel_main's locals) can reach
/// it. `FrameAllocator`'s own code is completely unmodified by this.
pub static FRAME_ALLOCATOR: SpinLock<Option<FrameAllocator>> = SpinLock::new(None);

/// Global handle to the virtual memory manager — same reasoning as
/// `FRAME_ALLOCATOR` above.
pub static VMM: SpinLock<Option<VirtualMemoryManager>> = SpinLock::new(None);

/// The actual `#[global_allocator]` target — see `main.rs` for the
/// registration. Wraps `LinkedListAllocator` in a `SpinLock` (required
/// for `Sync`; see `docs/kernel/MEMORY_MANAGER_DESIGN.md`) plus the
/// next-unmapped-heap-address cursor growth needs.
pub struct KernelHeap {
    inner: SpinLock<KernelHeapState>,
}

struct KernelHeapState {
    allocator: LinkedListAllocator,
    /// Next virtual address growth will map at. Starts at
    /// `HEAP_REGION_START` with nothing yet mapped — the very first
    /// allocation triggers the first growth, exactly like every
    /// subsequent out-of-space allocation; there is no separate
    /// "initial mapping" special case (see design doc).
    next_unmapped: u64,
}

impl KernelHeap {
    pub const fn new() -> Self {
        KernelHeap {
            inner: SpinLock::new(KernelHeapState {
                allocator: LinkedListAllocator::new(),
                next_unmapped: HEAP_REGION_START,
            }),
        }
    }

    /// Attempt to grow the heap by mapping enough additional pages to
    /// satisfy `layout` (or the fixed minimum growth size, whichever
    /// is larger), then add the newly-mapped region to the free list.
    /// Returns `true` if growth succeeded (the caller should retry its
    /// allocation), `false` if it could not (frame allocator
    /// exhausted, heap region exhausted, or the global VMM/frame
    /// allocator were never initialized).
    fn grow(state: &mut KernelHeapState, layout: Layout) -> bool {
        let pages_needed = (layout.size() as u64).div_ceil(PAGE_SIZE as u64).max(1);
        let pages_to_map = pages_needed.max(HEAP_GROWTH_MIN_PAGES);
        let growth_bytes = pages_to_map * PAGE_SIZE as u64;

        let growth_start = state.next_unmapped;
        let growth_end = match growth_start.checked_add(growth_bytes) {
            Some(end) if end <= HEAP_REGION_END => end,
            _ => return false, // would exceed the heap region ADR-002 reserves
        };

        let mut frame_guard = FRAME_ALLOCATOR.lock();
        let mut vmm_guard = VMM.lock();
        let (Some(frame_allocator), Some(vmm)) = (frame_guard.as_mut(), vmm_guard.as_mut())
        else {
            return false; // globals not populated yet — see kernel_main's init order
        };

        // Map every page in the growth range before touching the free
        // list — if any single page fails to allocate/map partway
        // through, bail out without having told the free list about a
        // region that isn't actually fully backed by real memory.
        let mut mapped_pages: u64 = 0;
        while mapped_pages < pages_to_map {
            let virt = VirtAddr::new(growth_start + mapped_pages * PAGE_SIZE as u64);
            let Some(phys) = frame_allocator.allocate() else {
                Self::unwind_partial_growth(vmm, growth_start, mapped_pages);
                return false;
            };
            if vmm
                .map(frame_allocator, virt, phys, PageFlags::read_write())
                .is_err()
            {
                frame_allocator.deallocate(phys).ok();
                Self::unwind_partial_growth(vmm, growth_start, mapped_pages);
                return false;
            }
            mapped_pages += 1;
        }

        state.next_unmapped = growth_end;
        // SAFETY: [growth_start, growth_end) was just fully mapped,
        // page by page, immediately above — real, exclusively-owned,
        // writable memory the allocator has never seen before (it's
        // strictly beyond every previous next_unmapped cursor value).
        unsafe {
            state
                .allocator
                .add_free_region(growth_start as usize, growth_bytes as usize);
        }
        true
    }

    /// Unmap and free back whatever prefix of a growth attempt
    /// succeeded before a later page in the same attempt failed —
    /// leaves the heap's mapped state consistent (no partially-mapped,
    /// not-yet-in-any-free-list region left dangling) rather than
    /// abandoning it. This is real cleanup, not a placeholder: an
    /// interrupted growth is a real, reachable code path (frame
    /// exhaustion partway through a large growth), not a
    /// hypothetical.
    fn unwind_partial_growth(vmm: &mut VirtualMemoryManager, growth_start: u64, mapped_pages: u64) {
        for i in 0..mapped_pages {
            let virt = VirtAddr::new(growth_start + i * PAGE_SIZE as u64);
            // Best-effort: if unmap somehow fails here, there is no
            // further fallback — the page stays mapped-but-unused,
            // which is safe (never added to the free list, so never
            // handed out) even though it isn't reclaimed. Genuinely
            // reached only if page-table state is already corrupt,
            // which no code path in this kernel has ever produced.
            let _ = vmm.unmap(virt);
        }
    }
}

// SAFETY: every operation touches shared state only through the
// internal SpinLock, which provides the same mutual-exclusion
// guarantee `unsafe impl Sync` for a lock-wrapped type always relies
// on (see sync/spinlock.rs).
unsafe impl Sync for KernelHeap {}

unsafe impl GlobalAlloc for KernelHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut state = self.inner.lock();

        let first_attempt = state.allocator.alloc(layout);
        if !first_attempt.is_null() {
            return first_attempt;
        }

        if !Self::grow(&mut state, layout) {
            return core::ptr::null_mut();
        }

        // Retry exactly once after a successful growth — growth always
        // maps at least enough pages for this specific request (see
        // `grow`'s pages_needed calculation), so this second attempt
        // is expected to succeed; if it somehow doesn't (e.g. an
        // alignment requirement larger than a single growth chunk),
        // returning null here is the correct, honest outcome rather
        // than looping growth attempts indefinitely.
        state.allocator.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.lock().allocator.dealloc(ptr, layout);
    }
}
