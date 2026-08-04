# TODO

Future feature requests and ideas are recorded here instead of being
implemented early. Nothing in this file is scheduled — it exists so a
request isn't lost or forgotten, not as a queue to work through out of
phase order. See [ROADMAP.md](ROADMAP.md) for what's actually planned
and when, and [CONTRIBUTING.md](CONTRIBUTING.md)/[ADR process] for how
a TODO item eventually becomes real work: it gets proposed for a
specific phase, gets an ADR if it's architecturally significant, and
only then gets implemented — never added directly to a phase already
in progress.

## Recorded feature requests (not yet scheduled beyond their listed phase)

- **AI Assistant integrated into the OS** — Phase 13 per the original
  20-phase plan. Not started; no design work done yet.
- **Graphical desktop environment / window manager** — Phases 7-9.
  Not started.
- **Gaming APIs** (2D/3D acceleration hooks) — Phase 14. Not started.
- **Device drivers** beyond the bootloader/kernel's own minimal
  early-boot UART (keyboard, mouse, GPU framebuffer, disk, NIC) —
  Phase 4. Not started; `docs/adr/ADR-006-kernel-architecture.md`'s
  "Early debug output vs. Phase 4 drivers" section explains why the
  current serial code is NOT this.
- **Networking stack** — Phase 6. Not started.
- **Security model** (capability-based permissions, signed packages,
  process isolation) — Phase 15. Not started. Note: the current
  kernel panic policy (ADR-006) is "halt the whole machine" precisely
  because no process isolation exists yet to make anything less
  drastic meaningful — this is expected to change once Phase 15 (or
  the scheduler's process model, whichever lands first) exists.
- **Face unlock / biometric authentication** — not yet assigned to a
  specific phase in the original plan; would depend on Phase 15
  (Security) and Phase 4 (a camera/sensor driver) both existing first.
  Recorded here as a request, not committed to any phase yet.
- **Package manager** — Phase 11. Not started.
- **App store / software distribution beyond the package manager
  itself** — not in the original 20-phase plan; recorded as a request
  for consideration once Phase 11 (Package Manager) is further along.
- **Cloud services integration** — Phase 16. Not started.
- **Compiler toolchain** (the OS's own, for building userland
  programs) — Phase 12. Not started.

## Known follow-ups from completed subsystems (smaller-scoped than the above)

- **Buddy allocator or other faster physical frame allocation
  strategy** — explicitly deferred to Phase 18 (Optimization) per
  `docs/kernel/MEMORY_MANAGER_DESIGN.md`; the current bitmap allocator
  is correct and boot-tested but O(n) worst-case by design for now.
- **Frame allocator bitmap sizing** — see `TECH_DEBT.md` for the
  specific, already-observed inefficiency (the bitmap currently sizes
  itself off the highest address in the ENTIRE memory map, including
  MMIO regions, not just usable RAM).
- **`core::fmt` / `write!`-style formatting for kernel debug output**
  — noted as a natural follow-up in `kernel/src/arch/x86_64/serial.rs`,
  not implemented since nothing has needed more than
  `write_str`/`write_hex64` yet.
- **Floating-point/SSE state in the scheduler's saved context** —
  `docs/kernel/SCHEDULER_DESIGN.md` explicitly defers this until some
  task actually needs floating point; the kernel target spec disables
  SSE entirely for now so this isn't a silent gap.
