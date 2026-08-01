# Original OS Project

An operating system built completely from scratch — no Linux, Windows,
macOS, or BSD source code anywhere in the tree. Where we must interoperate
with external standards (x86_64 architecture, UEFI, ACPI, USB, PCIe), we
implement those *specifications* independently from their public
documentation, never from an existing reference implementation.

Current status: **Phase 2 (Bootloader), Part 2 of ~4 complete.** See
[ROADMAP.md](ROADMAP.md) for the full phase plan and
[CHANGELOG.md](CHANGELOG.md) for what has actually landed.

## What exists right now

A UEFI PE32+ bootloader that boots under OVMF/QEMU and reads a real
file off the boot volume via the Simple File System Protocol —
`LoadedImageProtocol → SimpleFileSystemProtocol → OpenVolume → Open →
GetInfo → AllocatePool → Read` — printing its contents to the console
to prove the pipeline end to end. This is a genuine, verified
checkpoint — not a placeholder — see [docs/adr](docs/adr) for the
architecture decisions behind it.

## Quick start

```bash
# 1. Install the toolchain — see toolchain/SETUP.md for full details
sudo apt-get install mingw-w64 nasm qemu-system-x86 ovmf

# 2. Build the bootloader
cd boot && make

# 3. Boot-test it in QEMU
mkdir -p ../build/esp/EFI/BOOT
cp ../build/BOOTX64.EFI ../build/esp/EFI/BOOT/BOOTX64.EFI
cp testdata/BOOTINFO.TXT ../build/esp/BOOTINFO.TXT
cp /usr/share/OVMF/OVMF_CODE_4M.fd ../build/OVMF_CODE.fd
cp /usr/share/OVMF/OVMF_VARS_4M.fd ../build/OVMF_VARS.fd
qemu-system-x86_64 -machine q35 -m 256M \
  -drive if=pflash,format=raw,readonly=on,file=../build/OVMF_CODE.fd \
  -drive if=pflash,format=raw,file=../build/OVMF_VARS.fd \
  -drive format=raw,file=fat:rw:../build/esp \
  -serial stdio -display none -no-reboot
```

## Project structure

```
/boot          UEFI bootloader (C, freestanding, PE32+)
/kernel        Kernel core (Rust, no_std) and drivers (C11)
/toolchain     Cross-compiler setup and target specs
/userland      Userland programs (from Phase 9 onward)
/docs          Architecture Decision Records, vision, design docs
/tests         Automated boot/unit tests
.github/       CI workflows: build, static analysis, QEMU boot test, release
```

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) — system architecture overview
- [docs/VISION.md](docs/VISION.md) — project vision and v1 feature scope
- [docs/adr/](docs/adr) — all Architecture Decision Records (frozen once accepted; see CONTRIBUTING.md for the amendment process)
- [ROADMAP.md](ROADMAP.md) — phase-by-phase plan, 20 phases total
- [CONTRIBUTING.md](CONTRIBUTING.md) — how the project is developed
- [SECURITY.md](SECURITY.md) — how to report vulnerabilities

## Versioning

This project follows [Semantic Versioning](https://semver.org/). Current
version: see [VERSION](VERSION). While the major version is `0`, the API
and every on-disk/ABI format should be considered unstable and subject to
change without notice, per SemVer's own definition of the `0.y.z` range.

## License

MIT — see [LICENSE](LICENSE).
