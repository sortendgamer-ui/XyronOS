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

- **AI Assistant integrated into the OS** ("NeoAI") — Phase 13 per the
  original 20-phase plan. Not started; no design work done yet.
- **Graphical desktop environment / window manager** — Phases 7-9.
  Not started.
- **Gaming APIs** (2D/3D acceleration hooks) — Phase 14. Not started.
- **Device drivers** beyond the bootloader/kernel's own minimal
  early-boot UART (keyboard, mouse, GPU framebuffer, disk, NIC) —
  Phase 4. Not started; `docs/adr/ADR-006-kernel-architecture.md`'s
  "Early debug output vs. Phase 4 drivers" section explains why the
  current serial code is NOT this.
- **Networking stack** ("Neo Connect") — Phase 6. Not started.
- **Security model** ("Neo Defender" — capability-based permissions,
  signed packages, process isolation) — Phase 15. Not started. Note:
  the current kernel panic policy (ADR-006) is "halt the whole
  machine" precisely because no process isolation exists yet to make
  anything less drastic meaningful — this is expected to change once
  Phase 15 (or the scheduler's process model, whichever lands first)
  exists.
- **Face unlock / biometric authentication** — not yet assigned to a
  specific phase in the original plan; would depend on Phase 15
  (Security / "Neo Defender") and Phase 4 (a camera/sensor driver)
  both existing first. Recorded here as a request, not committed to
  any phase yet.
- **Package manager** ("Neo Store") — Phase 11. Not started.
- **App store / software distribution beyond the package manager
  itself** ("Neo Store") — not in the original 20-phase plan; recorded
  as a request for consideration once Phase 11 (Package Manager) is
  further along.
- **Update mechanism** ("Neo Update") — not yet assigned to a specific
  phase; would depend on Phase 11 (Package Manager) existing first.
- **File manager / explorer application** ("Neo Explorer") — Phase 10
  (Built-in Applications). Not started.
- **Settings / system configuration application** ("Neo Settings") —
  Phase 10 (Built-in Applications). Not started.
- **Web browser** ("Neo Browser") — Phase 10 (Built-in Applications)
  at minimum a basic HTML/HTTP renderer per `docs/VISION.md`'s v1
  scope; a full engine is out of scope for v1. Not started.
- **Cloud services integration** — Phase 16. Not started.
- **Compiler toolchain** (the OS's own, for building userland
  programs) — Phase 12. Not started.

*Note on branding:* the names above (NeoAI, Neo Store, Neo Browser,
Neo Defender, Neo Connect, Neo Update, Neo Explorer, Neo Settings) are
the official product names decided as part of the project's rename
from XyronOS to NeoastrenOS. None of these features exist yet — the
names are recorded now so the correct name is used from each
feature's first commit, rather than requiring a rename later.

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
- **Verify hardware enforcement of `NO_EXECUTE`.** `WRITABLE`
  enforcement is now verified (v0.6.0, Interrupts and Exceptions
  subsystem: a deliberate write to a page mapped `writable: false`
  produced a real page fault with error code `0x3` — present + write
  — and `CR2` matching the exact address, decoded and reported
  correctly by the new exception handlers). `NO_EXECUTE` enforcement
  (attempting to execute code on a page mapped `no_execute: true`)
  remains unverified — a natural, low-risk follow-up now that page
  fault handling exists, not attempted this milestone to keep its
  self-test focused on GDT/IDT/exception-handling correctness itself.
- **Verify the double-fault IST stack switch actually engages under a
  real kernel stack overflow.** `kernel/src/arch/x86_64/gdt.rs`
  correctly wires up the TSS's IST entry and the double-fault IDT
  entry's IST index (real code, not a placeholder — see
  `docs/kernel/INTERRUPTS_DESIGN.md`), but deliberately causing a
  kernel stack overflow to confirm the CPU really does switch to that
  dedicated stack rather than faulting again on the overflowed one was
  not attempted this milestone — safely triggering it without risking
  corrupting other kernel state (which would make the rest of the
  self-test's own output unreliable) needs more care than this
  subsystem's other checks.
- **Kernel heap allocator improvements** — see `TECH_DEBT.md` for full
  detail: a slab/size-class layer to reduce small-allocation internal
  fragmentation, and a shrink pass to unmap and return fully-unused
  heap pages to the physical frame allocator. Both explicitly deferred
  to Phase 18 (Optimization), matching the frame allocator's own
  deferred-optimization precedent.
- **`SpinLock` contention testing** — see `TECH_DEBT.md`. Cannot be
  meaningfully tested until interrupts and/or the scheduler create any
  genuine concurrent execution context; revisit once either exists.
