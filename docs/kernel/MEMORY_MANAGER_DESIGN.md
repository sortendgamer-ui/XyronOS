# Kernel Memory Manager — Design

Status: **Physical frame allocator implemented and boot-tested
(`kernel/src/mm/`). Virtual memory manager implemented and boot-tested
this milestone. Kernel heap: designed, not yet implemented** — see
ADR-006's initialization order for why it comes later, as its own
subsystem part.

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

## What this document does not cover
- Per-process address spaces (separate page table sets per process) —
  that is scheduler/process-model territory
  (`SCHEDULER_DESIGN.md`), built on top of this virtual memory
  manager's `map`/`unmap` primitives once processes exist.
- Swapping/paging to disk — no filesystem exists yet (Phase 5); not a
  Phase 3 concern.
- NUMA, huge-page-backed heap, or any allocator performance work —
  Phase 18.
