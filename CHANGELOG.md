# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]
### Planned (Phase 3)
- Kernel scheduler, memory manager, interrupts, exceptions, syscalls,
  timers (Rust `no_std` core, per ADR-001).

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
