# ADR-003: Bootloader File I/O via UEFI Simple File System Protocol

## Status
Accepted — 2026-07-31

## Context
The bootloader needs to read files (starting with a test file in Part 2,
the kernel image itself from Part 4 onward) off the EFI System
Partition, which is FAT12/16/32 formatted per the UEFI spec's ESP
requirement. There are two ways to do this:

1. Call `EFI_BLOCK_IO_PROTOCOL` directly for raw sector access and parse
   the FAT filesystem structures ourselves.
2. Use `EFI_SIMPLE_FILE_SYSTEM_PROTOCOL` (SFSP), a firmware-provided,
   UEFI-spec-mandated protocol that already implements Open/Read/Close
   file semantics over whatever filesystem driver firmware has bound to
   the volume (in practice, always FAT for the ESP).

This decision was implicit in the Phase 1 ADRs' choice of UEFI as the
sole boot path, but had not been written down explicitly until Part 2
needed it, so it is recorded now as its own ADR rather than folded into
ADR-001 or ADR-002, both of which are frozen and unrelated to this
specific question.

## Decision
Use `EFI_SIMPLE_FILE_SYSTEM_PROTOCOL` for all bootloader file I/O. Do
not write a FAT12/16/32 parser as part of the bootloader.

This is consistent with the project's no-existing-OS-code rule: SFSP is
a documented UEFI *service interface*, in the same category as
`AllocatePages` or `ExitBootServices` — calling it is calling a
specified firmware API, not incorporating source code from an existing
bootloader or OS. The rule against existing OS code targets copying
implementations (e.g. a Linux or GRUB FAT driver), not the use of
interfaces the UEFI specification itself defines as the standard way to
do this.

## Consequences
- The bootloader has zero FAT-parsing code and zero dependency on any
  particular FAT variant (12 vs. 16 vs. 32) — firmware's own filesystem
  driver handles that entirely.
- The bootloader's file I/O is limited to whatever SFSP exposes:
  synchronous Open/Read/GetInfo/Close on a volume firmware has already
  mounted. This is sufficient for reading a kernel image and any boot
  configuration files planned through the rest of Phase 2, and no
  planned future part requires more.
- If a future phase ever needs to read a non-FAT filesystem the
  bootloader must access *before* the kernel is running (not a
  currently planned requirement — normal OS filesystems in Phase 5 are
  a kernel-level concern, not a bootloader one), that would require a
  new ADR superseding this one, since SFSP only covers what firmware's
  bound filesystem driver supports.

## Alternatives Considered
- **Own FAT12/16/32 parser over EFI_BLOCK_IO_PROTOCOL:** rejected for
  Phase 2. Would be more "from scratch" in spirit, but adds significant
  code and testing surface (three FAT variants, cluster chains, long
  filename entries) for a component (the bootloader) whose only job is
  to get the kernel loaded and get out of the way. Revisitable later if
  a concrete need for BLOCK_IO-level control emerges — not needed today.
