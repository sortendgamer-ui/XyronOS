# Kernel Memory Manager — Design

Status: **Physical frame allocator implemented and boot-tested
(`kernel/src/mm/`). Virtual memory manager implemented and boot-tested.
Kernel heap allocator implemented and boot-tested this milestone —
the Memory Manager subsystem is now complete.**

## Goals
1. Turn the raw UEFI memory map handed off in `BootInfo`
   (`MemoryMapPhysAddr` etc., ADR-005) into a usable physical frame
   allocator.
2. Provide a kernel heap (`GlobalAlloc` implementation) so kernel code
   can use `alloc::{Box, Vec, ...}` once initialized — required before
   any subsystem that needs dynamic allocation (the scheduler's task
   list, for one) can be built.
3. Extend the bootloader-built page tables (identity map + higher-half
   kernel region, ADR-005) with the kernel's own virtual memory
   management, since the bootloader's tables were only ever meant to
   get the kernel started, not to serve as the kernel's permanent
   memory map.

## Physical frame allocator: bitmap
One bit per 4 KiB physical frame, `0` = free, `1` = used. Built by
walking the `BootInfo` memory map once at init:
- Frames covering `EfiConventionalMemory` regions start free.
- Every other region type (firmware-reserved, ACPI, MMIO, and
  crucially the frames already consumed by the kernel image, the
  bootloader-built page tables, the memory map buffer itself, and the
  kernel stack — all of which appear in the map as whatever type they
  were allocated as, typically `EfiLoaderData`/`EfiLoaderCode`) starts
  marked used.
- The bitmap's own backing storage is placed in the first large-enough
  free `EfiConventionalMemory` region found while walking the map, and
  that region's own frames are then marked used for the bitmap itself,
  in the same pass — a bitmap that didn't reserve its own storage
  would let something else be allocated on top of it.

**Concrete constraint, settled during implementation:** the region
chosen for the bitmap's storage must lie entirely below `0x100000000`
(4 GiB) — the limit of the identity map the bootloader's page tables
already cover (ADR-005). The virtual memory manager (next kernel
subsystem, not yet built) is what will eventually let the kernel map
*any* physical address; until then, the kernel can only directly
dereference a physical address that already falls inside the
bootloader's identity map, so the bitmap — which the kernel must write
to directly during this very subsystem's init — has nowhere else it
could safely live. This does not limit which frames the bitmap can
*track or allocate* (the full physical memory map is tracked,
including anything above 4 GiB) — only where the bookkeeping structure
itself is stored. Frames above 4 GiB can be allocated by this
subsystem (correct bookkeeping), but are not yet directly usable by
kernel code until the virtual memory manager can map them — a known,
documented limitation of this subsystem alone, not a defect (see
`TECH_DEBT.md`).

A bitmap (versus a free-list) is the deliberate choice for this first
implementation: O(n) worst-case allocation (scanning for a free bit)
is acceptable at this stage — the kernel has no workload yet that
stresses allocator performance — and a bitmap is far simpler to get
correct than a free-list's pointer bookkeeping. Revisiting this for a
faster allocator (buddy system, for compile-time-known power-of-two
allocation sizes) is explicitly deferred to Phase 18 (Optimization),
not implemented speculatively now.

API surface: `alloc_frame() -> Option<PhysAddr>`,
`free_frame(addr: PhysAddr)`, plus a bulk `alloc_frames(count)` for
callers that need contiguous physical memory (page-table pages do
not — see below — but some future driver DMA buffer might).

## Virtual memory manager
Wraps page-table manipulation behind a safe(r) API rather than having
every caller poke raw page-table entries:
- `map(virt: VirtAddr, phys: PhysAddr, flags: PageFlags) -> Result<(), MapError>`
- `unmap(virt: VirtAddr) -> Result<PhysAddr, MapError>`
- `translate(virt: VirtAddr) -> Option<PhysAddr>`

Unlike the bootloader's `paging.c` (2 MiB pages only, two fixed
regions), the kernel's virtual memory manager uses standard 4 KiB
pages and supports mapping arbitrary virtual addresses — needed once
the kernel starts managing per-allocation mappings (heap growth,
eventually per-process address spaces in the scheduler's process
model). It reuses the same PML4 the bootloader built and activated
(no second CR3 switch at this stage) and extends it — the bootloader's
identity map and higher-half kernel mapping remain valid and are not
torn down; new mappings are added alongside them.

Page-table pages themselves come from the physical frame allocator
above, one frame at a time as new intermediate tables are needed —
this is where the frame allocator's simple "any free frame" allocation
(no alignment stronger than 4 KiB required, unlike the bootloader's
2 MiB requirement) is sufficient, no huge-page alignment logic needed
here.

**Concrete decisions, settled during implementation:**

- **`VirtAddr` mirrors `PhysAddr`'s newtype pattern** (`mm/virt_addr.rs`):
  a wrapped `u64` with methods decomposing it into the four 9-bit
  page-table indices (PML4/PDPT/PD/PT) plus a 12-bit page offset, per
  the x86_64 architecture's 4-level paging layout — an external
  hardware fact, not a design choice, same category as `elf.h`'s
  structures or the UEFI struct definitions in `boot/`.
- **`PageTableEntry` (`mm/page_table_entry.rs`) wraps the raw `u64`
  entry format** (Present/Writable/NoExecute bits, physical address in
  bits 51:12) per the x86_64 architecture's documented page-table
  entry layout (Intel SDM Vol. 3A / AMD64 APM Vol. 2) — independently
  implemented from the specification, the same relationship
  `boot/paging.c` already has to this same hardware fact.
- **`PageFlags` currently exposes `WRITABLE` and `NO_EXECUTE`.** No
  `USER`/`SUPERVISOR` distinction yet — no ring-3 code exists to make
  it meaningful (that is scheduler/process-model territory,
  `SCHEDULER_DESIGN.md`, not this subsystem).
- **`NO_EXECUTE` requires `EFER.NXE` to be set, or the CPU treats bit
  63 as reserved-must-be-zero and raises a page-fault-adjacent
  reserved-bit violation the instant such an entry is walked.** UEFI
  firmware does not reliably guarantee this bit is already enabled —
  relying on it being enabled by coincidence would be exactly the kind
  of unverified assumption the project's rules prohibit. The virtual
  memory manager's `init` therefore reads the `EFER` MSR and sets the
  NXE bit itself (idempotent — a no-op if firmware already set it)
  before any `NO_EXECUTE` mapping can be created.
- **`map`/`unmap` invalidate the TLB entry for the affected virtual
  address (`invlpg`) after modifying the page table.** Without this,
  the CPU may continue using a stale cached translation for that
  address after the page table itself has already changed — a subtle,
  easy-to-miss correctness requirement, not an optional optimization.
- **Page-table physical addresses (CR3, and every intermediate table
  pointer the walker follows) are checked against
  `IDENTITY_MAP_LIMIT` before being dereferenced, and `map`/`unmap`/
  `translate` return an error rather than dereferencing an
  out-of-range address.** This exists because `boot/paging.c` allocates
  its own page-table pages via unconstrained `AllocatePages(AllocateAnyPages, ...)`
  — not capped below `IDENTITY_MAP_LIMIT` the way Part 4's *other*
  allocations were (kernel image, memory map buffer, BootInfo, kernel
  stack; see ADR-005). In every observed boot (QEMU/OVMF) these pages
  have landed well under 4 GiB, and `boot/` is frozen — per this
  project's own rule, unmodified unless an actual bug is found, and
  none has been found: nothing has ever actually failed. This is
  therefore recorded as a documented, currently-theoretical risk in
  `TECH_DEBT.md`, not fixed in `boot/paging.c` speculatively — but the
  kernel's own new code does not blindly trust the assumption either;
  it checks and fails cleanly if the assumption is ever violated.
- **Testing split, matching the frame allocator's own precedent:**
  `VirtAddr`'s index decomposition and `PageTableEntry`'s bit
  manipulation are pure logic, host-unit-tested. `map`/`unmap`/
  `translate`'s actual page-table manipulation requires real physical
  memory and the real frame allocator, so — like `FrameAllocator::init`
  itself — it is verified via a boot-time integration self-test
  instead (round-trip map → write through the mapping → read back →
  unmap → confirm `translate` now returns `None`).
- **What is implemented but NOT verified this milestone, stated
  explicitly rather than assumed:** `NO_EXECUTE` and the absence of
  `WRITABLE` are set correctly in the page table entry, and
  `translate()` can be used to confirm the stored flags round-trip
  correctly. Whether the CPU actually *enforces* them (traps a write
  to a non-writable page, traps execution of a no-execute page) cannot
  be verified yet — that requires a page-fault exception handler,
  which does not exist until the next kernel subsystem (interrupts/
  exceptions). The boot self-test states this limitation in its own
  output rather than silently omitting the check.

## Kernel heap allocator
A single, kernel-wide `GlobalAlloc` implementation backed by a fixed
virtual region in the kernel-heap portion of the address space
(`0xFFFF880000000000` onward, per ADR-002), grown on demand: when an
allocation request cannot be satisfied by already-mapped heap space,
the allocator asks the virtual memory manager to map additional 4 KiB
pages (backed by fresh frames from the physical allocator) at the next
unused heap address, then satisfies the request from the newly
available space.

The allocation algorithm within that mapped region is a linked-list
allocator (first-fit): a free list threaded through the free blocks
themselves (no separate metadata array — each free block's own first
bytes store its size and a pointer to the next free block). This is
the standard, well-understood starting point for a kernel heap — not
the fastest possible design, but correct and simple to reason about,
consistent with the same "correctness first, optimize in Phase 18"
principle as the frame allocator's bitmap choice above.

`#[global_allocator]` is registered once, in `main.rs`, pointing at
this allocator — after which `alloc::{Box, Vec, String, ...}` become
usable throughout the kernel.

**Concrete decisions, settled during implementation:**

- **Split into two layers**, matching the testing-split precedent
  every prior part of this subsystem established: `mm/heap.rs`'s
  `LinkedListAllocator` is pure free-list logic operating over any
  `[start, start+size)` byte range it's given — no dependency on the
  VMM, frame allocator, or real hardware, so it is fully
  host-unit-tested (a `Vec<u8>`-backed buffer stands in for real
  mapped memory in tests). `KernelHeap` is the thin `GlobalAlloc`
  wrapper that adds growth-on-demand (calling the VMM/frame
  allocator) on top of it — that half needs real hardware and is
  boot-tested, like `FrameAllocator::init` and the VMM's `map`/`unmap`
  before it.
- **Free list is kept sorted by address, with adjacency coalescing on
  free.** A non-coalescing free list (the simplest possible version)
  was considered and rejected: without merging adjacent free blocks,
  memory freed in small pieces can never satisfy a later request
  larger than any single piece, even when those pieces are physically
  contiguous — a real, user-visible degradation over time, not merely
  a performance concern, and exactly the kind of shortcut this
  project's rules ask to avoid. Coalescing here is a plain
  address-order check (does this freed block's end equal the next
  list entry's start, or the previous entry's end equal this block's
  start?), not a boundary-tag scheme — no extra per-block bookkeeping
  needed beyond what an address-sorted list already has.
- **Minimum block size and alignment.** Every block — free or
  allocated — must be at least `size_of::<FreeListNode>()` bytes and
  aligned to at least `align_of::<FreeListNode>()`, since a freed
  block stores its own `FreeListNode` header in its first bytes.
  `alloc()` rounds every request up to satisfy this before searching
  the free list; `dealloc()` recomputes the identical rounding from
  the `Layout` it's given (the same technique `GlobalAlloc`'s API
  shape already assumes: whatever `Layout` was passed to `alloc` is
  guaranteed passed back to `dealloc`, so both sides derive the same
  reserved size independently rather than needing it stored anywhere
  extra).
- **Leftover gaps smaller than one block header are lost, not
  reclaimed — stated plainly, not silently accepted.** Fitting a
  request into a free block can leave a leftover gap on either side:
  in front, if the block's raw address isn't already aligned to what
  the allocation needs; behind, if the block is larger than the
  request. Either gap, if large enough to hold its own
  `FreeListNode`, is kept as a new, separate free block — no loss. If
  smaller than that, there's nowhere valid to record it as free, and
  those few bytes become permanent internal fragmentation for the
  lifetime of the heap. This is a well-known, accepted limitation of
  simple linked-list allocators generally, not specific to this
  implementation — recorded in `TECH_DEBT.md` rather than glossed
  over.
- **Growth chunk size: `max(pages the failing request needs, 16
  pages)`, rounded up to a whole number of 4 KiB pages.** Mapping
  exactly one page per growth would work but calls the VMM far more
  often than necessary for a heap under sustained use; a fixed 64 KiB
  (16-page) minimum amortizes that cost while staying small enough
  that a lightly-used kernel doesn't reserve excessive physical memory
  up front. Growth stops (allocation fails, `alloc()` returns null)
  if the heap's cursor would cross `KERNEL_VIRTUAL_BASE`
  (`0xFFFFFFFF80000000`) — the top of the heap region ADR-002 reserves
  — rather than silently mapping into the kernel image's own address
  range.
- **`FRAME_ALLOCATOR` and `VMM` become kernel-wide globals
  (`SpinLock<Option<T>>`), populated once in `kernel_main` after their
  existing boot self-tests run using local bindings exactly as
  before.** This is required because `GlobalAlloc::alloc`/`dealloc`
  take only `&self` and a `Layout` — there is no parameter through
  which `kernel_main`'s local `frame_allocator`/`vmm` variables could
  otherwise reach a global allocator's growth logic. Neither
  `FrameAllocator` nor `VirtualMemoryManager`'s own code changes at
  all — their public APIs, internal logic, and every existing unit
  test are completely unmodified; `kernel_main` only changes how it
  *stores* the already-built instances afterward. `VirtualMemoryManager::map`'s
  existing signature (`&mut self, allocator: &mut FrameAllocator, ...`)
  is used completely unchanged — heap growth locks both globals and
  passes the already-locked `FrameAllocator` reference straight
  through, so there is no double-locking or re-entrancy concern.
- **A minimal spin lock (`kernel/src/sync/spinlock.rs`) is introduced**
  — required because a `static` (needed for `#[global_allocator]` and
  the two globals above) must be `Sync`, and this kernel has no
  OS-provided synchronization primitive to reach for. No real
  contention exists yet (no interrupts, no second CPU, no scheduler —
  this kernel runs strictly single-threaded through everything built
  so far), so this cannot be contention-tested this milestone; it is
  still implemented correctly (atomic compare-exchange, not a
  placeholder) because every subsystem after this one (interrupts,
  the scheduler) will need real mutual exclusion and this is the
  correct primitive for that, not a stand-in for something better
  later. `sync/` joins `arch/` and `mm/` in the module tree — ADR-006's
  module layout already anticipates new subsystem-driven directories
  being added exactly this way, so no ADR amendment is needed for it.

## What this document does not cover
- Per-process address spaces (separate page table sets per process) —
  that is scheduler/process-model territory
  (`SCHEDULER_DESIGN.md`), built on top of this virtual memory
  manager's `map`/`unmap` primitives once processes exist.
- Swapping/paging to disk — no filesystem exists yet (Phase 5); not a
  Phase 3 concern.
- NUMA, huge-page-backed heap, or any allocator performance work —
  Phase 18.
