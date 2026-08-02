# ADR-005: Kernel Handoff — Calling Convention, BootInfo ABI, and Paging

## Status
Accepted — 2026-07-31

## Context
Part 4 must transfer control from the bootloader to a kernel entry
point in a way that is correct, not merely one that happens to work in
QEMU. Three separate problems have to be solved together, because they
constrain each other:

1. **Calling convention mismatch.** The bootloader is built with the
   mingw-w64 cross-compiler (ADR-001), which defaults to the Microsoft
   x64 ABI (first integer argument in RCX) for every function,
   matching what UEFI itself requires. The kernel core will be Rust
   `no_std` targeting `x86_64-unknown-none` (ADR-001), whose default
   (and the industry-standard convention for bare-metal x86_64 code)
   is the System V AMD64 ABI (first integer argument in RDI). A naive
   `jmp` to the kernel entry point after setting up a single argument
   in the "obvious" register would pass that argument in the wrong
   register for whichever side wasn't expecting it.
2. **BootInfo needs a stable, self-describing layout.** The bootloader
   and kernel are compiled independently (different toolchains
   entirely), so the struct handing off memory map and kernel location
   information between them cannot rely on both sides being built from
   the same header at the same time forever — it needs a magic number
   and version field so a mismatched pair fails loudly instead of
   reading garbage.
3. **Paging must be correct at the exact instant of the jump.** UEFI
   leaves us in long mode under firmware's own page tables, which do
   not map the kernel's higher-half virtual addresses from ADR-002.
   We must build new page tables, switch to them, and only then jump —
   and the switch itself is hazardous: the very next instruction
   fetch after loading CR3 must still resolve correctly.

## Decision

**Calling convention boundary.** The bootloader-to-kernel jump is
implemented in NASM (`boot/trampoline.asm`), consistent with ADR-001's
allocation of "lowest-level entry points" to assembly. This trampoline
is called from C using the bootloader's native MS x64 ABI (arguments
arrive in RCX/RDX/R8), and internally moves the BootInfo pointer into
RDI before jumping — satisfying the kernel's SysV AMD64 ABI
expectation. This one small file is the entire calling-convention
boundary; neither side needs to know the other's ABI beyond it.

**BootInfo ABI** (`boot/include/boot_info.h`): a flat struct beginning
with a magic number (`BOOTINFO_MAGIC`) and a version integer
(`BOOTINFO_VERSION`), so a kernel receiving this struct can validate it
before trusting any other field, and so this struct can gain new
fields in a future version without silently breaking a kernel built
against an older one (a version bump, checked at kernel startup, is
required whenever a field is added, removed, or reordered).

**Paging** (`boot/paging.c`): the bootloader builds a fresh 4-level
page table using 2 MiB pages exclusively (no 4 KiB page tables, no 1
GiB pages) covering exactly two regions:
- An identity map (virtual == physical) of the first 4 GiB of physical
  address space, so that (a) the trampoline's own code keeps executing
  correctly immediately after the CR3 switch, and (b) any physical
  address the kernel receives via BootInfo (which are physical
  addresses, not pre-mapped virtual ones) is dereferenceable as-is.
- The higher-half kernel mapping at `0xFFFFFFFF80000000` per ADR-002,
  sized to whatever the loaded kernel image actually needs (rounded up
  to a 2 MiB boundary).

To guarantee every physical address the kernel needs to dereference
falls inside that 4 GiB identity-mapped region, every boot-time
allocation whose address the kernel will see directly (the kernel's
own loaded pages, the final memory map buffer, and the BootInfo struct
itself) uses `AllocatePages` with `AllocateMaxAddress` capped at `0x100000000`
(4 GiB), never `AllocatePool` or unconstrained `AllocateAnyPages` — an
allocator that returns memory anywhere in the address space would risk
returning something above our identity map's coverage, which would
fault the instant the kernel dereferenced it.

**Kernel image format:** ELF64, `ET_EXEC` only (no PIE/`ET_DYN`
support — the kernel is linked at a single fixed address per ADR-002,
so relocation support adds complexity with no benefit here),
`EM_X86_64`. Parsed against the ELF64 structures in `boot/include/elf.h`,
written from the ELF specification, not copied from an existing
loader.

## Consequences
- `memory_map.c`'s final memory-map-buffer allocation changes from
  `AllocatePool` (used in Part 3) to `AllocatePages` with
  `AllocateMaxAddress`. This is an implementation refinement to
  already-written Part 3 code, not a reversal of any ADR — no prior
  ADR specified `AllocatePool` as a decision; it was simply the
  simplest correct choice until Part 4's reachability requirement
  existed. Part 3's stale-map-key retry loop itself is unchanged.
- A kernel larger than 1 GiB cannot be loaded by this bootloader as
  written (the higher-half PD table covers at most 512 × 2 MiB = 1 GiB
  starting at `0xFFFFFFFF80000000`). This is an explicit, documented
  limit, not an oversight — no realistic kernel image approaches this
  size, and extending to multiple PDPT entries if that ever changes is
  a small, isolated change to `paging.c` alone.
- The bootloader now contains its first NASM code
  (`boot/trampoline.asm`), which ADR-001 always anticipated for
  "lowest-level entry points" but Parts 1-3 had no occasion to need.

## Alternatives Considered
- **Passing BootInfo in RCX and letting the kernel adapt:** rejected —
  would require the kernel's entry point to be written against the
  bootloader's ABI instead of Rust's natural bare-metal default,
  pushing an unnecessary constraint onto every future kernel change.
- **4 KiB page tables throughout:** rejected for this bootloader-built
  initial mapping — correct but needlessly larger (thousands of PT
  entries instead of a handful of PD entries) for a mapping that
  exists only to get the kernel started; the kernel's own memory
  manager (Phase 3) will build its real, fine-grained page tables
  once it takes over.
- **Identity-mapping all of physical RAM instead of a fixed 4 GiB
  cap:** rejected — requires knowing total RAM size before building
  the table (a chicken-and-egg problem solvable but unnecessary), and
  4 GiB comfortably covers every allocation this bootloader makes.
