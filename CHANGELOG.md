# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]
### Planned (Phase 2, Parts 2-4)
- Full `EFI_BOOT_SERVICES` table definition.
- Simple File System Protocol reader (own FAT parsing, no firmware
  FAT driver dependency beyond the standard protocol interface).
- Memory map retrieval and `ExitBootServices` handoff.
- Kernel image loading and jump-to-kernel handoff struct.

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
