# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]
### Planned (Phase 3, next subsystem)
- Interrupts/exceptions: GDT, IDT, exception handlers.

## [v0.5.0] - 2026-08-07
### Phase 3 (Kernel) — Memory Manager subsystem COMPLETE: kernel heap allocator implemented, tested, and boot-verified.
### Added
- `kernel/src/mm/linked_list_allocator.rs` — `LinkedListAllocator`:
  pure, hardware-independent free-list allocation logic (address-sorted
  free list with adjacency coalescing on both sides, minimum block size
  derived from `FreeListNode`'s own layout, first-fit search with
  front/back gap splitting). 10 unit tests, including three dedicated
  coalescing tests (two adjacent blocks, order-independence, three
  adjacent blocks merging into one).
- `kernel/src/sync/spinlock.rs`, `kernel/src/sync/mod.rs` — `SpinLock`:
  a minimal atomic compare-exchange spin lock, needed because
  `#[global_allocator]` and the new `FRAME_ALLOCATOR`/`VMM` globals all
  require `Sync`. 3 unit tests. New `sync/` module, joining `arch/` and
  `mm/` — no ADR amendment needed since ADR-006's module layout already
  anticipates subsystem-driven directories added this way.
- `kernel/src/mm/heap.rs` — `KernelHeap`: the real `GlobalAlloc`
  implementation. Growth-on-demand (minimum 16 pages / 64 KiB per
  growth step, capped at `KERNEL_VIRTUAL_BASE` so the heap region never
  collides with the kernel's own image), backed by two new kernel-wide
  globals (`FRAME_ALLOCATOR`, `VMM`) that `kernel_main` populates once
  their own boot self-tests complete — `FrameAllocator` and
  `VirtualMemoryManager`'s own code is completely unmodified; only how
  `kernel_main` stores the already-built instances changes.
- `#[global_allocator]` registered in `main.rs` — `alloc::{Box, Vec,
  String, ...}` are now usable throughout the kernel.
- Boot-time integration self-test (`run_heap_boot_self_test`): a
  `Box<u64>` round-trip in the correct heap region; 100 small
  allocations checked pairwise-distinct with intact values; a
  20,000-element `Vec<u32>` (forcing multiple internal reallocations —
  proving heap growth works repeatedly, not just once) with a
  checksum verification; repeated large alloc/free churn proving freed
  space is actually reused, not just accumulating unbounded growth.
- Design decisions settled before implementation, per instruction:
  `docs/kernel/MEMORY_MANAGER_DESIGN.md`'s Kernel Heap Allocator
  section gained a "Concrete decisions" subsection covering the
  two-layer split (pure logic vs. hardware-backed growth), the
  coalescing decision and why a non-coalescing list was rejected (real
  fragmentation, not just a performance concern), minimum block
  size/alignment handling, growth chunk sizing, and the
  `FRAME_ALLOCATOR`/`VMM` globals requirement.
- `TECH_DEBT.md`: three new entries — leftover alignment/sizing gaps
  smaller than one block header are permanently lost (accepted,
  documented limitation of simple linked-list allocators generally);
  heap growth never shrinks back (pages stay mapped for the kernel's
  lifetime even once fully freed); `SpinLock` has never been
  contention-tested (no concurrent execution context exists yet in
  this kernel to test it against).
- `TODO.md`: corresponding follow-ups recorded (slab/size-class layer
  and heap shrink pass, both deferred to Phase 18; SpinLock contention
  testing, deferred until interrupts/scheduler exist).
### Verified
- Real kernel build: zero warnings. Test build: zero warnings.
- Unit tests: 35/35 passing on host target (up from 21) — 10 new for
  `LinkedListAllocator`, 3 new for `SpinLock`, 21 prior tests
  unchanged and still passing.
- Full boot chain verified end to end under QEMU/OVMF against the
  **unmodified** Phase 2 bootloader: frame allocator, virtual memory
  manager, AND kernel heap allocator self-tests all passed in
  sequence — the Memory Manager subsystem's full, final verification.
- `boot/` has zero changes this milestone — frozen, no bug found
  requiring a change. No prior subsystem's own code
  (`frame_allocator.rs`, `vmm.rs`, `page_table_entry.rs`,
  `virt_addr.rs`, `phys_addr.rs`) was modified either — only how
  `kernel_main` stores their already-built instances afterward.

## [Project Rename] - 2026-08-06
### Renamed the project from XyronOS to NeoastrenOS — branding-only, no functional or architectural change.
No `VERSION` bump: nothing about the system's behavior, build output,
or boot sequence changed — verified by a full clean rebuild and boot
test after the rename, with output identical to the pre-rename run
except for the brand text itself. See `git log` for the exact commit;
this entry has no corresponding version tag for that reason (see
`ROADMAP.md`'s versioning note).
### Changed
- Every occurrence of "XyronOS" replaced with "NeoastrenOS" across all
  git-tracked files (verified via full-repository search: bootloader
  boot-screen and serial output strings in `boot/main.c`, the test
  fixture text in `boot/testdata/BOOTINFO.TXT`, the kernel's own boot
  banner in `kernel/src/main.rs`, the stub kernel fixture's banner in
  `tests/kernel_stub/kernel_stub.c`, and the kernel package's name/
  author/description in `kernel/Cargo.toml`, which also renames the
  compiled binary from `xyronos-kernel` to `neoastrenos-kernel` —
  every reference to that binary path/name updated to match across
  `README.md`, `kernel/README.md`, and all four CI workflow files).
- `TODO.md` — the future product brand names decided as part of this
  rename (NeoAI, Neo Store, Neo Browser, Neo Defender, Neo Connect,
  Neo Update, Neo Explorer, Neo Settings) recorded against the
  relevant not-yet-implemented feature requests, so each is built
  under its correct name from its first commit rather than needing a
  rename later. None of these features exist yet — nothing was
  actually renamed here, only named in advance.
### Verified unchanged
- `README.md`, `ROADMAP.md`, `ARCHITECTURE.md`, `CONTRIBUTING.md`,
  `SECURITY.md`, `CODE_OF_CONDUCT.md`, `LICENSE`, and every file under
  `docs/adr/` and `docs/kernel/` — a full-repository search confirmed
  none of these ever contained the string "XyronOS" (the project was
  referred to generically, e.g. "this project," "the OS," throughout),
  so nothing needed changing in them. ADR numbering, semantic version
  history (VERSION, existing git tags), and all prior CHANGELOG
  entries are unmodified.
- Full clean rebuild (bootloader + kernel), 21/21 unit tests, and a
  full QEMU/OVMF boot test all re-verified after the rename, with
  identical pass/fail results to the pre-rename v0.4.0 state.
### Known limitation
- The GitHub repository itself
  (`github.com/sortendgamer-ui/XyronOS`) cannot be renamed from this
  environment — that requires the repository owner's action on
  GitHub (Settings → repository name) and updating any local `git
  remote` URL afterward. Everything renameable from within the
  repository's own contents has been renamed; the external repo name
  is outside this change's reach and is not claimed to be done.

## [v0.4.0] - 2026-08-04
### Phase 3 (Kernel) — Memory Manager subsystem: virtual memory manager implemented, tested, and boot-verified.
### Added
- `kernel/src/mm/virt_addr.rs` — `VirtAddr` newtype, decomposing an
  address into its four 9-bit page-table indices (PML4/PDPT/PD/PT)
  plus a 12-bit page offset, per the x86_64 4-level paging layout.
  5 unit tests, including a cross-check against ADR-005's own by-hand
  PML4=511/PDPT=510 derivation for `KERNEL_VIRTUAL_BASE`.
- `kernel/src/mm/page_table_entry.rs` — `PageTableEntry` (raw page
  table entry bit wrapper: Present/Writable/NoExecute, physical
  address in bits 51:12) and `PageFlags`. 5 unit tests.
- `kernel/src/mm/vmm.rs` — `VirtualMemoryManager`: `init`/`map`/
  `unmap`/`translate`/`flags_at`, a 4-level page-table walker that
  creates intermediate tables on demand, reusing the bootloader's
  existing PML4 (no second CR3 switch). Sets `EFER.NXE` itself rather
  than assuming firmware already did (required for `NO_EXECUTE` to be
  meaningful, not reserved-bit-violate). Invalidates the TLB (`invlpg`)
  after every `map`/`unmap`. Every page-table physical address is
  checked against `IDENTITY_MAP_LIMIT` before being dereferenced.
- `FrameAllocator::allocate_below()` — new capability (not a change to
  existing `allocate()`/`deallocate()` behavior) letting the VMM
  request a frame it can directly dereference for a new page-table
  page. 2 new unit tests.
- Boot-time integration self-test (`run_vmm_boot_self_test` in
  `main.rs`): `translate()` verified against the kernel's own running
  code (proving the walker correctly handles the bootloader's existing
  2 MiB huge-page higher-half mapping — "higher-half kernel mapping
  compatibility," verified concretely); a fresh 4 KiB mapping created,
  written through, read back, its stored permission flags checked via
  `flags_at()`, then unmapped; both `map()`-already-mapped and
  `unmap()`-already-unmapped error paths exercised.
- Design decisions settled before implementation, per instruction:
  `docs/kernel/MEMORY_MANAGER_DESIGN.md`'s Virtual Memory Manager
  section gained a "Concrete decisions" subsection covering all of the
  above plus what is explicitly NOT verified this milestone (hardware
  enforcement of `WRITABLE`/`NO_EXECUTE` — requires a page-fault
  handler that doesn't exist until the next subsystem; stated, not
  silently skipped).
- `TECH_DEBT.md`: new entry documenting that `boot/paging.c`'s own
  page-table pages are allocated via unconstrained
  `AllocatePages(AllocateAnyPages, ...)`, not capped below
  `IDENTITY_MAP_LIMIT` the way Part 4's other allocations are. Never
  observed to actually cause a failure; the bootloader is frozen and
  not modified speculatively — the VMM instead defensively checks
  every page-table address it encounters and fails cleanly rather than
  assuming the address is reachable.
### Verified
- Real kernel build: zero warnings. Test build: zero warnings.
- Unit tests: 21/21 passing on host target (up from 9).
- Full boot chain verified end to end under QEMU/OVMF against the
  **unmodified** Phase 2 bootloader: kernel jumped to at
  `0xFFFFFFFF80001110`, CR3 `0xE655000`, every frame-allocator AND
  virtual-memory-manager self-test check passed, including the
  higher-half-compatibility translate() check against the kernel's own
  running code.
- `boot/` has zero changes this milestone — frozen, no bug found
  requiring a change.

## [v0.3.0] - 2026-08-02
### Phase 3 (Kernel) — Memory Manager subsystem: physical frame allocator implemented, tested, and boot-verified.
### Added
- `kernel/src/mm/phys_addr.rs` — `PhysAddr` newtype (compiler-enforced
  physical/virtual address distinction, per
  `docs/kernel/CODING_STANDARDS.md`'s type-system-safety guidance),
  frame-number conversion, alignment checking. 3 unit tests.
- `kernel/src/mm/memory_map.rs` — `MemoryMapIter`, reading the raw UEFI
  memory map from `BootInfo` at fixed byte offsets with
  `descriptor_size` as the iteration stride — the same non-fixed-
  stride lesson `boot/main.c` already applied, now also applied
  kernel-side.
- `kernel/src/mm/frame_allocator.rs` — bitmap physical frame allocator
  (`FrameAllocator::init`/`allocate`/`deallocate`), with upfront
  whole-map validation and per-entry validation (zero-length,
  non-frame-aligned, overflow-on-end-address) per requirement 3
  ("validate all memory regions before use"). 6 unit tests, including
  a double-free rejection test and a bookkeeping regression test.
- `kernel/src/mm/mod.rs` — subsystem module root.
- `kernel_main` (main.rs): memory manager bring-up (ADR-006 boot flow
  step 3) plus a boot-time integration self-test
  (`run_frame_allocator_boot_self_test`) exercising the allocator
  against the REAL memory map this specific boot received — 16
  allocations checked pairwise-distinct, bookkeeping checked after
  allocation and after freeing everything. Requirement 7.
- `main.rs` restructured with `#[cfg(not(test))]`/`#[cfg(test)]`
  gating so `mm/`'s unit tests can run on the host target while the
  real kernel stays a freestanding `no_std`/`no_main` binary.
- `TODO.md` — every future feature request (AI assistant, GUI, gaming,
  drivers, networking, security, face unlock, package manager, app
  store, etc.) recorded here rather than implemented early, per
  explicit instruction.
- `TECH_DEBT.md` — known, documented limitations: the frame allocator
  bitmap sizes itself off the entire memory map including MMIO
  regions (correct but wasteful; fix deferred, not a defect), plus the
  two `.cargo/config.toml` traps below.
### Fixed
- **`.cargo/config.toml`: `[unstable] build-std` applied globally,
  breaking `cargo test`.** Building `core` from source (build-std) for
  a host-target test run, while also linking the host's own prebuilt
  `core` (via prebuilt `std`), produced `error[E0152]: duplicate lang
  item`. Fixed by passing `-Z build-std=core,alloc` explicitly on the
  command line for real builds instead of via `[unstable]` — never for
  `cargo test`. Caught by actually running `cargo test`, not by
  inspection.
- **`.cargo/config.toml`: `rustflags` under `[build]` applied
  globally, corrupting the host test binary.** The kernel's
  `-Tlinker.ld` (entry point `0xFFFFFFFF80000000`) was being linked
  into the HOST test executable too, which then segfaulted immediately
  on startup with zero output. Fixed by scoping `rustflags` to
  `[target.x86_64-os]` instead of `[build]`. Also caught by actually
  running the tests — both fixes are documented in detail in
  `kernel/README.md` and `TECH_DEBT.md`.
### Changed
- `docs/kernel/MEMORY_MANAGER_DESIGN.md` — status updated from
  "designed, not yet implemented" to reflect the physical frame
  allocator's completion; added the concrete below-4-GiB bitmap
  storage constraint settled during implementation (identity map
  limit, ADR-005).
- `kernel/README.md` — full rewrite: correct build-std/rustflags
  invocation, unit-test-running instructions, and the two config.toml
  traps explained in detail.
- `.github/workflows/build.yml` — added `test-kernel` job (runs the
  host-target unit tests); every kernel `cargo` invocation across all
  workflows updated with the now-explicit `-Z build-std` flags.
- `.github/workflows/qemu-boot-test.yml`, `.github/workflows/release.yml`
  — success marker updated to `"Frame allocator boot self-test: ALL
  CHECKS PASSED"` (the new, more specific verification point).
### Verified
- Real kernel build: zero warnings, clean release build.
- Unit tests: 9/9 passing on host target
  (`x86_64-unknown-linux-gnu`).
- Full boot chain re-verified end to end under QEMU/OVMF after every
  fix in this milestone, against the unmodified Phase 2 bootloader and
  unmodified Phase 3 skeleton entry sequence — zero changes to `boot/`
  this milestone.

## [v0.2.0] - 2026-08-02
### Phase 3 (Kernel) started — architecture designed, first buildable skeleton verified booting.
### Added
- `docs/adr/ADR-006-kernel-architecture.md` — monolithic kernel
  decision (formalizing what ADR-001's driver/core split already
  implied), kernel module layout, boot flow/initialization order,
  panic policy, and the early-debug-output-vs-Phase-4-drivers
  distinction.
- `docs/kernel/MEMORY_MANAGER_DESIGN.md` — bitmap physical frame
  allocator, page-table-backed virtual memory manager, linked-list
  kernel heap allocator. **Designed, not yet implemented** — the next
  kernel subsystem part builds this.
- `docs/kernel/SCHEDULER_DESIGN.md` — task model, context-switch
  mechanism (callee-saved registers only, no FP/SSE state), and a
  round-robin scheduling algorithm. **Designed, not yet implemented.**
- `docs/kernel/CODING_STANDARDS.md` — `unsafe` usage rules, panic vs.
  `Result` policy, formatting/lint enforcement, testing approach.
- `kernel/src/boot_info.rs` — the Rust half of the ADR-005 `BootInfo`
  ABI contract, `#[repr(C)]`, field-for-field mirror of
  `boot/include/boot_info.h`.
- `kernel/src/main.rs`, `kernel/src/arch/x86_64/serial.rs` — the first
  buildable Rust `no_std` kernel skeleton: validates `BootInfo`
  (magic/version/size), reports its own physical/virtual location and
  the received memory map summary over an independently-implemented
  COM1 UART driver, then halts.
- `toolchain/x86_64-os.json` — custom freestanding x86_64 target spec
  (no built-in triple assumes an OS underneath, per Phase 1's original
  toolchain plan).
- `kernel/Cargo.toml`, `kernel/.cargo/config.toml`, `kernel/linker.ld`,
  `kernel/rust-toolchain.toml` — kernel build configuration; places the
  kernel at `KERNEL_VIRTUAL_BASE` (`0xFFFFFFFF80000000`, ADR-002).
- `kernel/README.md` — build/boot-test instructions, including a
  documented apt-based toolchain workaround for environments that
  cannot reach `rustup`'s own domain (this project's own sandbox
  needed it: `RUSTC_BOOTSTRAP=1`, a one-time `Cargo.lock` generation
  for apt's `rust-src` package, and swapping a broken `rust-lld`
  symlink for `ld.lld`) — CI uses the standard `rustup` path instead.
### Changed
- `.github/workflows/build.yml` — real (not speculative) kernel build
  job; verifies the output is a valid ELF64 executable.
- `.github/workflows/static-analysis.yml` — real `clippy`/`rustfmt`
  enforcement for kernel code (`-D warnings`).
- `.github/workflows/qemu-boot-test.yml` — split into two jobs:
  bootloader + `tests/kernel_stub` fixture (fast bootloader regression
  check, no Rust toolchain needed) and bootloader + the real kernel
  (the actual Phase 3 milestone verification).
- `.github/workflows/release.yml` — builds and boot-verifies the real
  kernel before packaging a release, plus a regression check against
  the fixture kernel to confirm the bootloader (frozen since Phase 2)
  hasn't changed behavior.
### Verified
- Full chain booted live under QEMU/OVMF with the bootloader completely
  unmodified from its Phase 2 v0.1.0 state: real Rust kernel reached
  `kernel_main` at `0xFFFFFFFF80000190`, validated `BootInfo`, and
  printed every handed-off field (kernel physical/virtual base, size,
  stack, memory map entry count and descriptor size) correctly before
  halting.

## [v0.1.0] - 2026-07-31
### Phase 2 (Bootloader) complete.
### Added
- `docs/adr/ADR-005-kernel-handoff.md` — calling-convention boundary
  (MS x64 bootloader / SysV AMD64 kernel), the versioned `BOOT_INFO`
  ABI, and the paging strategy (identity map + higher-half mapping,
  both via 2 MiB pages) used to make the jump correct.
- `boot/include/boot_defs.h` — shared constants (page sizes, identity
  map limit, kernel virtual base, kernel size limit).
- `boot/include/elf.h` — ELF64 structures (header, program header)
  per the ELF specification.
- `boot/include/boot_info.h` — the `BOOT_INFO` handoff struct: magic,
  version, memory map descriptor, kernel image location, dedicated
  kernel stack.
- `boot/include/paging.h`, `boot/paging.c` — builds a fresh 4-level
  page table: identity map of the first 4 GiB (2 MiB pages) plus the
  higher-half kernel mapping at `0xFFFFFFFF80000000` (ADR-002).
- `boot/include/kernel_loader.h`, `boot/kernel_loader.c` — validates
  and loads an `ET_EXEC` ELF64 kernel image into a capped-address
  physical location, correctly handling BSS (zero-fill) and using
  the ELF's own `e_phentsize` as the program-header iteration stride
  (not `sizeof(Elf64_Phdr)`).
- `boot/trampoline.asm` — the first NASM code in this project
  (per ADR-001): switches CR3, switches to the dedicated kernel stack,
  moves the BootInfo pointer into RDI (SysV ABI), and jumps to the
  kernel entry point.
- `tests/kernel_stub/` — a minimal ELF64 test fixture (explicitly NOT
  Phase 3 kernel work; see its README.md) that validates the received
  BootInfo and reports success over serial, proving the full handoff
  end to end.
### Changed
- `boot/memory_map.c` — final memory map buffer allocation switched
  from `AllocatePool` to address-capped `AllocatePages`
  (`AllocateMaxAddress`), so the kernel can dereference it post-jump
  (ADR-005). The stale-map-key retry loop itself is unchanged.
- `boot/main.c` — orchestrates the full sequence: file read (Part 2,
  unchanged) → load kernel ELF → allocate + pre-populate BootInfo →
  allocate kernel stack → build page tables → retrieve memory map and
  exit boot services (Part 3, unchanged) → finish populating BootInfo
  → jump to kernel. Verified end to end under QEMU/OVMF: kernel stub
  reached its entry point at `0xFFFFFFFF80000000`, validated BootInfo,
  and printed every handed-off field correctly.
- `boot/Makefile` — builds and links `paging.c`, `kernel_loader.c`,
  and assembles/links `trampoline.asm` (NASM, win64 object format).
### Fixed
- **Use-after-free in `kernel_loader.c`:** `OutKernel->EntryPointVirtual`
  was read from `ehdr->e_entry` (a pointer into `fileBuffer`) AFTER
  `fileBuffer` had already been freed. Caught by actually booting the
  full chain in QEMU — the entry point printed as
  `0xAFAFAFAFAFAFAFAF`, EDK2's freed-pool debug scrub pattern. Fixed
  by capturing `e_entry` into a local variable before the `FreePool`
  call. Documented in the source as a demonstration of why this
  project boot-tests every part rather than only compiling it.

## [v0.0.3-alpha] - 2026-07-31
### Added
- `docs/adr/ADR-004-post-exit-diagnostics.md` — decision to use a
  bootloader-local raw 16550 UART driver (COM1) for diagnostics after
  `ExitBootServices`, since `ConOut` is not guaranteed valid past that
  point.
- `boot/include/serial.h`, `boot/serial.c` — minimal 16550 UART driver:
  explicit baud/line-control/FIFO initialization, no dependency on
  firmware having configured the port.
- `boot/include/memory_map.h`, `boot/memory_map.c` — spec-correct
  `GetMemoryMap` + `ExitBootServices` sequencing with the required
  stale-map-key retry loop (bounded at 5 attempts), plus buffer-size
  slack to avoid a second `EFI_BUFFER_TOO_SMALL` from the allocation's
  own effect on the memory map.
### Changed
- `boot/main.c` — Part 2's file-read pipeline unchanged; on success,
  now retrieves the final memory map, calls `ExitBootServices` (with
  retry), and reports success over raw COM1 — including a full memory
  map walk (using firmware-reported descriptor stride, not
  `sizeof(EFI_MEMORY_DESCRIPTOR)`) that sums usable (Conventional)
  memory. Verified under QEMU/OVMF: 130 memory map entries, 48-byte
  descriptor stride, ExitBootServices succeeded on first attempt.
- `boot/Makefile` — builds and links `serial.c` and `memory_map.c`.
- `.github/workflows/qemu-boot-test.yml`, `.github/workflows/release.yml`
  — assertions updated to the Part 3 completion marker.

## [v0.0.2-alpha] - 2026-07-31
### Added
- `docs/adr/ADR-003-bootloader-file-io.md` — decision to use
  `EFI_SIMPLE_FILE_SYSTEM_PROTOCOL` for all bootloader file I/O rather
  than writing an own FAT12/16/32 parser.
- `boot/include/efi_boot_services.h` — full `EFI_BOOT_SERVICES` function
  table (44 entries), spec-accurate field order and signatures.
- `boot/include/efi_loaded_image_protocol.h` — `EFI_LOADED_IMAGE_PROTOCOL`,
  used to discover the bootloader's own source volume.
- `boot/include/efi_file_protocol.h` — `EFI_SIMPLE_FILE_SYSTEM_PROTOCOL`
  and `EFI_FILE_PROTOCOL` definitions.
- `boot/testdata/BOOTINFO.TXT` — test fixture proving the file-read
  pipeline end to end.
### Changed
- `boot/main.c` — now acquires `LoadedImageProtocol` →
  `SimpleFileSystemProtocol` → opens root volume → opens, reads, and
  prints a test file's contents. Verified byte-for-byte correct under
  QEMU/OVMF.
- `.github/workflows/qemu-boot-test.yml`, `.github/workflows/release.yml`
  — updated to stage the test file on the ESP and assert on the Part 2
  success marker.

## [v0.0.1-alpha] - 2026-07-31
### Added
- Repository initialized as a professional software project: `.gitignore`,
  MIT `LICENSE`, `README.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`,
  `SECURITY.md`, `ROADMAP.md`, `ARCHITECTURE.md`, `VERSION`.
- GitHub Actions workflows: build, static analysis, QEMU boot test,
  release packaging.
- **Phase 1 (Architecture) complete and frozen:**
  - `docs/VISION.md` — project vision and v1 feature scope.
  - `docs/adr/ADR-001-kernel-language.md` — Rust core / C drivers / NASM
    bootloader language split.
  - `docs/adr/ADR-002-memory-layout.md` — x86_64 higher-half virtual
    memory layout.
  - `toolchain/SETUP.md` — cross-compiler setup for Rust, C (ELF), and
    C via mingw-w64 (PE32+, bootloader-specific).
- **Phase 2 (Bootloader), Part 1 complete:**
  - `boot/include/efi_types.h` — UEFI fundamental scalar types and GUID.
  - `boot/include/efi_tables.h` — `EFI_SYSTEM_TABLE` and Simple Text
    Output Protocol definitions.
  - `boot/main.c` — UEFI entry point; verified booting under QEMU/OVMF,
    prints confirmation text to console and halts.
  - `boot/Makefile` — builds a verified valid PE32+ EFI application via
    the mingw-w64 cross-compiler.
