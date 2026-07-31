# ADR-002: Virtual Memory Layout (x86_64)

## Status
Accepted — 2026-07-31

## Context
x86_64 long mode gives us a 64-bit virtual address space, but current CPUs
only implement 48 usable bits (canonical addresses), giving 256 TiB total,
split into two 128 TiB halves by the canonical-address hole:
`0x0000_7FFF_FFFF_FFFF` (top of low half) and `0xFFFF_8000_0000_0000`
(bottom of high half). We adopt the common "higher-half kernel" design:
user space lives in the low half, kernel space in the high half, so a
single set of page tables (with the U/S bit controlling access) serves
both — no page table swap is needed on syscall/interrupt entry.

## Decision
```
0x0000_0000_0000_0000 - 0x0000_7FFF_FFFF_FFFF   User space          (128 TiB)
0xFFFF_8000_0000_0000 - 0xFFFF_87FF_FFFF_FFFF   Direct physical map (8 TiB)
0xFFFF_8800_0000_0000 - 0xFFFF_FFFF_7FFF_FFFF   Kernel heap region  (~120 TiB)
0xFFFF_FFFF_8000_0000 - 0xFFFF_FFFF_FFFF_FFFF   Kernel image        (2 GiB)
```

- **User space (low half):** every process gets its own set of page tables
  for this region; stack grows down from near the top, heap grows up from
  the binary's load address, standard OSDev layout.
- **Direct physical map:** all physical RAM is mapped once, linearly, at a
  fixed offset (`phys_addr + 0xFFFF_8000_0000_0000`). The memory manager
  uses this instead of temporary mappings to touch arbitrary physical
  pages (e.g. when walking another process's page tables) — avoids the
  complexity of recursive page-table mapping tricks.
- **Kernel heap:** dynamically mapped as the slab/buddy allocator (Phase 3)
  requests more pages. Not identity-mapped; managed like user heap but at
  ring 0.
- **Kernel image:** the kernel ELF itself is linked to load at
  `0xFFFF_FFFF_8000_0000`, giving it a 2 GiB addressable window — small
  enough to use x86_64's efficient RIP-relative addressing and the
  `-mcmodel=kernel` code model in both GCC and rustc.

## Consequences
- The linker script (written in Phase 2 alongside the bootloader) must
  place the kernel's entry point at `0xFFFFFFFF80000000 + <load offset>`.
- Any physical-to-virtual address translation in the memory manager is a
  single addition of the fixed offset — no page walk required for kernel
  code touching physical memory it already knows the address of.
- Because user and kernel share one address space layout (just gated by
  privilege level), context switches change CR3 (page table base) only
  when switching between different *processes*, not between user and
  kernel mode within the same process — this matters for syscall latency,
  revisited in ADR-005 (Syscall Path) in Phase 3.

## Alternatives Considered
- **Separate address spaces per privilege level with CR3 swap on every
  syscall:** rejected — meaningfully slower syscall path for no benefit
  once we're not sharing tables with unrelated processes.
- **48-bit direct map covering full range:** rejected — reserving 8 TiB is
  already far beyond any real machine's installed RAM for the foreseeable
  future; no need to reserve the full 128 TiB half for it.
