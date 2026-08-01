# Roadmap

Twenty phases, worked in order. A phase does not start until the previous
phase's code compiles and meets its stated success criteria. Status is
updated as each phase/part lands — see [CHANGELOG.md](CHANGELOG.md) for
the detailed history behind each checkmark.

| Phase | Name | Status |
|-------|------|--------|
| 1 | Requirements, Vision, Architecture, Boot/Memory Design | ✅ Complete & frozen |
| 2 | Bootloader (UEFI, disk loader, boot menu) | 🔵 In progress (Part 2/4) |
| 3 | Kernel (scheduler, memory manager, interrupts, syscalls, timers) | ⬜ Not started |
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

## Phase 2 breakdown (current)

- [x] Part 1 — Minimal UEFI PE32+ "hello world," verified booting in QEMU/OVMF
- [x] Part 2 — Full Boot Services table + Simple File System Protocol reader, verified reading a real file end to end
- [ ] Part 3 — Memory map retrieval + `ExitBootServices`
- [ ] Part 4 — Kernel image loading + handoff struct + jump to kernel

## Versioning approach across phases

Each completed part/milestone bumps the alpha version
(`v0.0.1-alpha` → `v0.0.2-alpha` → ...). The minor version moves to `0.1.0`
when Phase 2 (Bootloader) is fully complete and the bootloader can load
and hand off to a real kernel. Major version `1.0.0` is reserved for
Phase 20 (Stable Release), per [README.md](README.md)'s SemVer note.
