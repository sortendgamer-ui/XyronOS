# Original OS Project

An operating system built completely from scratch — no Linux, Windows,
macOS, or BSD source code anywhere in the tree. Where we must interoperate
with external standards (x86_64 architecture, UEFI, ACPI, USB, PCIe), we
implement those *specifications* independently from their public
documentation, never from an existing reference implementation.

Current status: **Phase 2 (Bootloader) complete. Phase 3 (Kernel) in
progress** — architecture designed, kernel skeleton verified booting,
physical frame allocator and virtual memory manager (memory manager
subsystem) implemented, tested, and boot-verified. See
[ROADMAP.md](ROADMAP.md) for the full phase plan and
[CHANGELOG.md](CHANGELOG.md) for what has actually landed. Future
feature requests not yet scheduled are tracked in [TODO.md](TODO.md);
known non-bug limitations in [TECH_DEBT.md](TECH_DEBT.md).

## What exists right now

A UEFI PE32+ bootloader (Phase 2, complete and frozen) that boots
under OVMF/QEMU, reads a real file off the boot volume via the Simple
File System Protocol, loads and validates a real ELF64 kernel image,
builds the page tables that image needs to run at its linked
higher-half address (`0xFFFFFFFF80000000`), retrieves the final UEFI
memory map, calls `ExitBootServices` with the spec's required retry
logic, and jumps to the kernel's entry point with a populated
`BOOT_INFO` handoff struct.

On top of that: a real Rust `no_std` kernel (`kernel/`, Phase 3, in
progress) that this same bootloader loads and jumps to. It validates
the `BootInfo` handoff struct, then brings up a physical frame
allocator (bitmap-based, built from the real UEFI memory map, with
9 unit tests and a boot-time integration self-test that exercises it
against this specific boot's real data) before reporting success over
serial and halting. The virtual memory manager, kernel heap, and
scheduler are fully designed
(`docs/kernel/MEMORY_MANAGER_DESIGN.md`,
`docs/kernel/SCHEDULER_DESIGN.md`) but not yet implemented — see
`docs/adr/ADR-006-kernel-architecture.md` for the overall kernel
architecture these build on.

`tests/kernel_stub` is a separate, minimal C test fixture (explicitly
**not** Phase 3 work — see that directory's README) used to verify the
bootloader in isolation, independent of kernel/ development.

See [docs/adr](docs/adr) for the architecture decisions behind all of
this.

## Quick start

```bash
# 1. Install the toolchain — see toolchain/SETUP.md and kernel/README.md
sudo apt-get install mingw-w64 nasm qemu-system-x86 ovmf gcc lld
rustup toolchain install nightly && rustup component add rust-src --toolchain nightly

# 2. Build the bootloader and the kernel
cd boot && make && cd ..
cd kernel && cargo +nightly build --release && cd ..

# 3. Boot-test the full chain in QEMU
mkdir -p build/esp/EFI/BOOT
cp build/BOOTX64.EFI build/esp/EFI/BOOT/BOOTX64.EFI
cp boot/testdata/BOOTINFO.TXT build/esp/BOOTINFO.TXT
cp kernel/target/x86_64-os/release/xyronos-kernel build/esp/KERNEL.ELF
cp /usr/share/OVMF/OVMF_CODE_4M.fd build/OVMF_CODE.fd
cp /usr/share/OVMF/OVMF_VARS_4M.fd build/OVMF_VARS.fd
qemu-system-x86_64 -machine q35 -m 256M \
  -drive if=pflash,format=raw,readonly=on,file=build/OVMF_CODE.fd \
  -drive if=pflash,format=raw,file=build/OVMF_VARS.fd \
  -drive format=raw,file=fat:rw:build/esp \
  -serial stdio -display none -no-reboot
```
See `kernel/README.md` for an apt-based toolchain workaround if
`rustup` isn't reachable in your environment, and for testing the
bootloader in isolation against `tests/kernel_stub` instead.

## Project structure

```
/boot          UEFI bootloader (C, freestanding, PE32+)
/kernel        Kernel core (Rust, no_std) and drivers (C11)
/toolchain     Cross-compiler setup and target specs
/userland      Userland programs (from Phase 9 onward)
/docs          Architecture Decision Records, vision, design docs
/tests         Automated boot/unit tests, incl. tests/kernel_stub
               (a Phase 2 test fixture — NOT the real Phase 3 kernel)
.github/       CI workflows: build, static analysis, QEMU boot test, release
```

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) — system architecture overview
- [docs/VISION.md](docs/VISION.md) — project vision and v1 feature scope
- [docs/adr/](docs/adr) — all Architecture Decision Records (frozen once accepted; see CONTRIBUTING.md for the amendment process)
- [ROADMAP.md](ROADMAP.md) — phase-by-phase plan, 20 phases total
- [TODO.md](TODO.md) — future feature requests, recorded not scheduled
- [TECH_DEBT.md](TECH_DEBT.md) — known limitations that are correct but not optimal
- [CONTRIBUTING.md](CONTRIBUTING.md) — how the project is developed
- [SECURITY.md](SECURITY.md) — how to report vulnerabilities

## Versioning

This project follows [Semantic Versioning](https://semver.org/). Current
version: see [VERSION](VERSION). While the major version is `0`, the API
and every on-disk/ABI format should be considered unstable and subject to
change without notice, per SemVer's own definition of the `0.y.z` range.

## License

MIT — see [LICENSE](LICENSE).
