# Architecture Overview

This document is a map, not the source of truth — every specific decision
lives in a frozen [ADR](docs/adr) and should be read there. This file
exists so a newcomer (or future-us, picking this back up months later)
can see how the pieces fit together before diving into individual records.

## System diagram (target end state, not yet fully built)

```
 ┌─────────────────────────────────────────────────────────┐
 │                     UEFI Firmware                        │
 └───────────────────────────┬───────────────────────────────┘
                              │ loads & calls EfiMain()
 ┌───────────────────────────▼───────────────────────────────┐
 │  Bootloader (boot/) — C, freestanding, PE32+ (ADR-001)     │
 │  Reads files via SFSP (ADR-003) → loads kernel ELF64 →     │
 │  builds page tables → retrieves memory map → exits boot    │
 │  services (retry-safe) → jumps to kernel entry (ADR-005).  │
 │  Post-exit diagnostics via raw UART (ADR-004). PHASE 2      │
 │  COMPLETE — verified end to end against tests/kernel_stub. │
 └───────────────────────────┬───────────────────────────────┘
                              │ handoff struct (memory map, kernel
                              │ location, dedicated stack — BOOT_INFO,
                              │ ADR-005).
 ┌───────────────────────────▼───────────────────────────────┐
 │  Kernel core (kernel/) — Rust, no_std (ADR-001/ADR-006)    │
 │  Skeleton verified booting (validates BootInfo, reports    │
 │  over serial, halts). Memory Manager, Scheduler, IPC,       │
 │  Syscall dispatch: DESIGNED (docs/kernel/), not yet built.  │
 │  Higher-half layout per ADR-002                            │
 │        │                                                  │
 │        ▼                                                  │
 │  Drivers (kernel/drivers/) — C11, C-ABI boundary (ADR-001) │
 │  Keyboard · Mouse · GPU fb · Disk · NIC — Phase 4, not      │
 │  started                                                    │
 └───────────────────────────┬───────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   Filesystem            Network Stack         Graphics Engine
   (Phase 5)              (Phase 6)              (Phase 7)
        │                     │                     │
        └─────────────────────┼─────────────────────┘
                              ▼
              Window Manager → Desktop → Apps
                 (Phases 8-10)
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
  Package Manager      Compiler Toolchain      AI Assistant
   (Phase 11)             (Phase 12)             (Phase 13)
```

## Key architectural decisions (see docs/adr/ for full rationale)

- **ADR-001 — Kernel language split:** Rust (`no_std`) for the kernel
  core, C11 for drivers behind a fixed C-ABI boundary, NASM for the
  bootloader's lowest-level pieces.
- **ADR-002 — Virtual memory layout:** Higher-half kernel at
  `0xFFFFFFFF80000000`, direct physical map at `0xFFFF800000000000`, user
  space in the low 128 TiB half. Single page-table set per process, no
  swap on syscall entry.
- **ADR-003 — Bootloader file I/O:** reads files off the ESP via
  `EFI_SIMPLE_FILE_SYSTEM_PROTOCOL` (firmware's own FAT driver, accessed
  through the standard UEFI protocol interface) rather than writing a
  bootloader-owned FAT12/16/32 parser.
- **ADR-004 — Post-ExitBootServices diagnostics:** after
  `ExitBootServices` succeeds, the bootloader can no longer rely on
  `ConOut`, so it uses a minimal, bootloader-local 16550 UART driver
  (COM1, raw port I/O) to confirm success and report the final memory
  map. This driver is not shared with the kernel's own future serial
  driver (Phase 4).
- **ADR-005 — Kernel handoff:** the bootloader (MS x64 ABI) hands off
  to the kernel (SysV AMD64 ABI) through a single NASM trampoline; a
  versioned `BOOT_INFO` struct carries the memory map, kernel image
  location, and a dedicated kernel stack; a fresh 4-level page table
  (2 MiB pages: identity map of the first 4 GiB + higher-half kernel
  mapping) is built and activated immediately before the jump.
- **ADR-006 — Kernel architecture:** monolithic (drivers share the
  kernel's address space and privilege level, per ADR-001's C-ABI
  driver boundary), a fixed subsystem initialization order (early
  serial → BootInfo validation → memory manager → interrupts/timer →
  scheduler → syscalls), and an unconditional-halt panic policy until
  a process model exists to make anything less drastic meaningful.

## Component boundaries

- **`boot/`** never assumes the kernel exists in memory in any particular
  format beyond what the bootloader itself defines in the handoff struct
  (finalized in Phase 2 Part 4). This keeps the bootloader/kernel contract
  explicit and testable independently.
- **`kernel/drivers/`** talk to the kernel core only through the C-ABI
  vtable interface ADR-001 anticipates, to be formalized in a new ADR
  when Phase 4 starts — never by reaching into kernel-core Rust
  structs directly.
- **Everything above the kernel** (filesystem, network, graphics, and up)
  runs as userland or kernel-adjacent services communicating through the
  syscall/IPC interface defined in Phase 3 — not through shared memory
  hacks or direct function calls across the layer boundary.
- **`kernel/mm/` and `kernel/sched/`** (not yet implemented) will
  follow `docs/kernel/MEMORY_MANAGER_DESIGN.md` and
  `docs/kernel/SCHEDULER_DESIGN.md` respectively — read those before
  touching either subsystem.

## Where to look for what

| Question | Where |
|---|---|
| "Why did we choose X?" | `docs/adr/` |
| "What's built so far?" | `CHANGELOG.md` |
| "What's next?" | `ROADMAP.md` |
| "How do I build/test it?" | `README.md`, component-level docs (e.g. `toolchain/SETUP.md`) |
| "What's the v1 feature scope?" | `docs/VISION.md` |
