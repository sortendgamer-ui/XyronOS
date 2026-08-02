# Kernel Stub — Phase 2 Part 4 Test Fixture

**This is not Phase 3.** This directory exists solely to give Phase 2
Part 4's kernel loader, page-table builder, and jump mechanism a real
ELF64 executable to load and transfer control to, so that "the kernel
handoff mechanism is implemented" (the Part 4 requirement) can be
verified against something real rather than asserted without proof.

When Phase 3 begins, the actual kernel is built under `kernel/` in
Rust (`no_std`), per ADR-001 — this stub is never extended into that
work; it stays exactly this small, or is deleted once Phase 3's real
kernel exists and CI points at that instead.

## What it does

`kernel_stub.c`'s `KernelEntry` function (the ELF entry point,
reached via `boot/trampoline.asm`):
1. Re-initializes the COM1 UART itself (a kernel should not assume a
   bootloader's device state persists correctly into its own
   lifetime — it owns its own drivers from the moment it starts).
2. Validates the `BOOT_INFO` struct's `Magic` and `Version` fields
   (see `boot/include/boot_info.h` and ADR-005) before trusting
   anything else in it.
3. Prints the kernel's own physical/virtual base, size, and the
   memory map entry count it received — over raw serial, proving the
   handoff data arrived correctly.
4. Halts.

## Why it's built with native gcc, not a dedicated x86_64-elf cross
compiler

ADR-001's toolchain setup documents building a proper `x86_64-elf-gcc`
cross-compiler for driver code. That build is a multi-step process
(build binutils, then gcc, targeting `x86_64-elf`) appropriate for
Phase 4's real driver work. For this test fixture, the host's native
`gcc` (which targets `x86_64-linux-gnu`) is used instead, in fully
freestanding mode (`-ffreestanding -nostdlib`, no libc, no CRT startup,
custom linker script) — the *output* is a bare ELF64 image
indistinguishable in structure from what a dedicated cross-compiler
would produce, since nothing hosted is linked in either way. This is a
pragmatic, explicitly-documented choice for a throwaway test fixture,
not a change to ADR-001's toolchain decision for real kernel/driver
code.

## Building and testing standalone

```bash
cd tests/kernel_stub
make                          # produces kernel_stub.elf
cp kernel_stub.elf ../../build/esp/KERNEL.ELF
```

Then boot-test the full bootloader + kernel handoff exactly as
described in the top-level README's QEMU instructions. Expected final
output (over serial) ends with:

```
PHASE 2 COMPLETE: kernel entry reached with valid BootInfo.
Halting (this is a Phase 2 Part 4 test fixture, not the real kernel).
```
