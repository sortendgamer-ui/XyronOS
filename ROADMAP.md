# Roadmap

Twenty phases, worked in order. A phase does not start until the previous
phase's code compiles and meets its stated success criteria. Status is
updated as each phase/part lands — see [CHANGELOG.md](CHANGELOG.md) for
the detailed history behind each checkmark.

| Phase | Name | Status |
|-------|------|--------|
| 1 | Requirements, Vision, Architecture, Boot/Memory Design | ✅ Complete & frozen |
| 2 | Bootloader (UEFI, disk loader, boot menu) | ✅ Complete |
| 3 | Kernel (scheduler, memory manager, interrupts, syscalls, timers) | 🔵 In progress (Memory Manager subsystem complete; interrupts next) |
| 4 | Device Drivers | ⬜ Not started |
| 5 | File System | ⬜ Not started |
| 6 | Networking Stack | ⬜ Not started |
| 7 | Graphics Engine | ⬜ Not started |
| 8 | Window Manager | ⬜ Not started |
| 9 | Desktop Environment | ⬜ Not started |
| 10 | Built-in Applications | ⬜ Not started |
| 11 | Package Manager | ⬜ Not started |
| 12 | Compiler Toolchain | ⬜ Not started |
| 13 | AI Assistant | ⬜ Not started |
| 14 | Gaming APIs | ⬜ Not started |
| 15 | Security | ⬜ Not started |
| 16 | Cloud Services | ⬜ Not started |
| 17 | Testing | ⬜ Not started |
| 18 | Optimization | ⬜ Not started |
| 19 | Documentation | ⬜ Not started |
| 20 | Stable Release | ⬜ Not started |

## Phase 3 breakdown (in progress)

Subsystems are worked in the order ADR-006's "Boot flow and
initialization order" establishes — each is not started until the one
before it is documented and internally consistent, per project rule.

- [x] Architecture — ADR-006, memory manager design, scheduler design,
  `BootInfo` Rust binding, module layout, coding standards (all
  documented before any kernel code was written)
- [x] Skeleton — first buildable Rust `no_std` kernel: validates
  `BootInfo`, reports its handoff data over serial, halts. Verified
  booting against the unmodified Phase 2 bootloader.
- [x] Memory manager — physical frame allocator: bitmap allocator
  built from the real UEFI memory map, validated per-entry and
  whole-map, 9 unit tests (host target) + a boot-time integration
  self-test against real memory map data.
- [x] Memory manager — virtual memory manager: 4-level page-table
  walker, `map`/`unmap`/`translate`/`flags_at`, reusing the
  bootloader's PML4, EFER.NXE enablement, TLB invalidation. 12 unit
  tests (host target) + a boot-time integration self-test including a
  higher-half-kernel-mapping-compatibility check.
- [x] Memory manager — kernel heap allocator: `LinkedListAllocator`
  (address-sorted, coalescing free list) wrapped as a real
  `GlobalAlloc`, growth-on-demand backed by the VMM and frame
  allocator. 13 new unit tests (10 allocator + 3 `SpinLock`) + a
  boot-time integration self-test (100 distinct small allocations, a
  20,000-element `Vec` forcing multiple growth cycles, alloc/free
  churn proving space reuse). **Memory Manager subsystem complete.**
- [ ] Interrupts/exceptions — GDT, IDT, exception handlers
- [ ] Timer
- [ ] Scheduler (`docs/kernel/SCHEDULER_DESIGN.md`)
- [ ] System calls

## Phase 2 breakdown (complete)

- [x] Part 1 — Minimal UEFI PE32+ "hello world," verified booting in QEMU/OVMF
- [x] Part 2 — Full Boot Services table + Simple File System Protocol reader, verified reading a real file end to end
- [x] Part 3 — Memory map retrieval + `ExitBootServices`, verified with a real retry-safe sequence and post-exit raw-serial confirmation
- [x] Part 4 — ELF64 kernel loader + `BootInfo` handoff + page tables + jump to kernel, verified end to end against a real test-fixture kernel (`tests/kernel_stub`)

Note: the original Phase 2 brief listed a boot menu as part of this
phase. It has been deferred to Phase 3 startup (or a later Phase 2
addendum if needed sooner) since it depends on keyboard input
(`EFI_SIMPLE_TEXT_INPUT_PROTOCOL`, only forward-declared so far — see
`boot/include/efi_tables.h`) that no part of Phase 2 needed until now,
and the phase's core deliverable — a working, verified kernel handoff
mechanism — is what Part 4 completes.

## Versioning approach across phases

Each completed part/milestone bumps the alpha version
(`v0.0.1-alpha` → `v0.0.2-alpha` → ...). The minor version moves to `0.1.0`
when Phase 2 (Bootloader) is fully complete and the bootloader can load
and hand off to a real kernel. Major version `1.0.0` is reserved for
Phase 20 (Stable Release), per [README.md](README.md)'s SemVer note.

Administrative or branding-only changes that don't alter behavior,
build output, or the boot sequence (e.g. the XyronOS → NeoastrenOS
project rename) do not bump `VERSION` — see `CHANGELOG.md`'s
`[Project Rename]` entry for that specific example.
