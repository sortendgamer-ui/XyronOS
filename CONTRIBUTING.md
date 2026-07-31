# Contributing

This project is developed phase by phase, following the order laid out in
[ROADMAP.md](ROADMAP.md). Please read that alongside this file before
proposing work — a contribution that jumps ahead of the current phase
(e.g. filesystem code while Phase 2 is still open) will be asked to wait.

## Core rules

1. **No existing OS source code.** Nothing may be copied or adapted from
   Linux, Windows, macOS, BSD, or any other existing operating system.
   External *specifications* (UEFI, ACPI, USB, PCIe, the x86_64 SDM) are
   the only permitted references, and must be implemented independently.
2. **No placeholder code.** Every function must do what it claims. If a
   feature isn't ready, it doesn't get committed stubbed out — it gets
   left for the part/phase where it's actually implemented.
3. **Every commit must compile.** CI enforces this (see
   `.github/workflows/build.yml`), but please verify locally first.
4. **Architecture Decision Records (ADRs) are frozen once accepted.**
   Do not edit an existing ADR's decision. If a later phase reveals a
   critical technical reason to change course, write a **new** ADR that:
   - references the ADR it supersedes,
   - explains the technical reason for the change,
   - is reviewed with the same scrutiny as the original.
5. **Comment the "why," not just the "what."** Every non-obvious piece of
   code should explain why it exists, especially where it encodes a
   requirement from an external spec (UEFI, ACPI, etc.) rather than a
   design choice we made ourselves.

## Development process

- Work proceeds one phase at a time, and within a phase, one part at a
  time. A phase's code must build and its stated success criteria (e.g.
  "boots in QEMU," "passes static analysis") must be met before the next
  phase starts.
- Each part/milestone gets a git tag and a packaged release per
  RELEASING below.
- Commit messages should reference the phase/part they belong to, e.g.
  `[Phase 2 Part 2] Implement Simple File System Protocol reader`.

## Code style

- **C (drivers, bootloader):** C11, `-Wall -Wextra` clean, freestanding
  flags as documented in each component's Makefile.
- **Rust (kernel core, from Phase 3):** `#![no_std]`, `rustfmt` default
  style, `clippy` clean.
- **Assembly (NASM):** one instruction per line, comment blocks explaining
  register usage at the top of every routine.

## Releasing

Every milestone produces, per project convention:
- a ZIP archive of the full repository state at that point,
- a SHA-256 checksum of that archive,
- release notes describing what changed,
- a file manifest listing every file in the archive.

See `.github/workflows/release.yml` for the automated process, and
`CHANGELOG.md` for the human-readable history.

## Reporting issues

Bugs and design questions are welcome as issues. Security vulnerabilities
should NOT be filed as public issues — see [SECURITY.md](SECURITY.md).
