# Kernel Memory Manager — Design

Status: **Designed, not yet implemented.** This document describes the
design the next kernel subsystem part will build; the skeleton
accompanying ADR-006 does not implement any of this yet — see that
ADR's "Boot flow and initialization order" for why memory management
comes immediately after BootInfo validation but is still a separate,
later part.

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
