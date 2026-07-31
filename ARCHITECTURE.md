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
 │  Finds kernel on disk → builds boot-info struct →          │
 │  ExitBootServices → jumps to kernel entry                  │
 └───────────────────────────┬───────────────────────────────┘
                              │ handoff struct (memory map, GOP fb, ACPI RSDP)
 ┌───────────────────────────▼───────────────────────────────┐
 │  Kernel core (kernel/) — Rust, no_std (ADR-001)            │
 │  Scheduler · Memory Manager · IPC · Syscall dispatch       │
 │  Higher-half layout per ADR-002                            │
 │        │                                                  │
 │        ▼                                                  │
 │  Drivers (kernel/drivers/) — C11, C-ABI boundary (ADR-001) │
 │  Keyboard · Mouse · GPU fb · Disk · NIC                    │
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

## Component boundaries

- **`boot/`** never assumes the kernel exists in memory in any particular
  format beyond what the bootloader itself defines in the handoff struct
  (finalized in Phase 2 Part 4). This keeps the bootloader/kernel contract
  explicit and testable independently.
- **`kernel/drivers/`** talk to the kernel core only through the C-ABI
  vtable interface formalized in ADR-004 (written when Phase 4 starts) —
  never by reaching into kernel-core Rust structs directly.
- **Everything above the kernel** (filesystem, network, graphics, and up)
  runs as userland or kernel-adjacent services communicating through the
  syscall/IPC interface defined in Phase 3 — not through shared memory
  hacks or direct function calls across the layer boundary.

## Where to look for what

| Question | Where |
|---|---|
| "Why did we choose X?" | `docs/adr/` |
| "What's built so far?" | `CHANGELOG.md` |
| "What's next?" | `ROADMAP.md` |
| "How do I build/test it?" | `README.md`, component-level docs (e.g. `toolchain/SETUP.md`) |
| "What's the v1 feature scope?" | `docs/VISION.md` |
