# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]
### Planned (Phase 2, Part 4)
- Kernel image loading and jump-to-kernel handoff struct.

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
