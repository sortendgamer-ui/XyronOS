# ADR-006: Kernel Architecture

## Status
Accepted — 2026-08-02

## Context
Phase 3 begins real kernel code. Before writing any of it, the
project's own rules require settling: what kind of kernel this is
(the answer is implied but never stated outright by ADR-001's
driver/core split), how it is organized into modules, what order
subsystems initialize in, and what happens when something goes wrong
during that initialization — all decisions that every subsystem
written after this one will depend on and that would be expensive to
change later.

This ADR is deliberately about structure and process, not about the
memory manager's or scheduler's internal algorithms — those have
their own design documents (`docs/kernel/MEMORY_MANAGER_DESIGN.md`,
`docs/kernel/SCHEDULER_DESIGN.md`) referenced below, kept separate so
this ADR stays about decisions that constrain the whole kernel, not
one subsystem's internals.

## Decision

### Kernel type: monolithic
ADR-001 already decided drivers are C11 code linked into the kernel
image behind a C-ABI boundary — not separate user-space server
processes. That is, by definition, a **monolithic kernel**: kernel
core and drivers share one address space and one privilege level
(ring 0). This ADR states that consequence explicitly rather than
leaving it implied, because it affects real decisions Phase 3 makes
now (a single set of kernel page tables, no IPC-for-drivers machinery,
no separate driver privilege model) that a microkernel design would
answer differently.

### Module layout
```
kernel/
  Cargo.toml
  rust-toolchain.toml        pins the nightly channel + rust-src
  linker.ld                  places the kernel at KERNEL_VIRTUAL_BASE
  src/
    main.rs                  #![no_std] #![no_main]; kernel_main() entry,
                              panic handler, top-level init sequence
    boot_info.rs              #[repr(C)] BootInfo — the Rust half of the
                              ADR-005 handoff ABI contract
    arch/
      mod.rs
      x86_64/
        mod.rs
        serial.rs             early-boot debug output only — see
                              "Early debug output vs. Phase 4 drivers"
                              below; NOT the formal driver model
        gdt.rs                (added when Phase 3's interrupts/
                              exceptions subsystem starts)
        idt.rs                (added alongside gdt.rs)
    mm/                        memory manager — see MEMORY_MANAGER_DESIGN.md;
                              added as its own subsystem after the skeleton
    sched/                     scheduler — see SCHEDULER_DESIGN.md; added
                              after interrupts/timers exist, since
                              preemption needs them
    syscall/                   (added when Phase 3's syscall subsystem starts)
```
Each subsystem directory is added when that subsystem's part begins —
consistent with "do not proceed to the next kernel subsystem until the
current one is documented and internally consistent" — not
pre-created empty as scaffolding, matching this project's standing
rule against placeholder files.

### Boot flow and initialization order
`kernel_main(boot_info: *const BootInfo) -> !` is reached via
`boot/trampoline.asm` (ADR-005), running on the dedicated stack and
page tables the bootloader built. Initialization proceeds in a fixed
order, each step depending only on steps before it:

1. Early debug serial output (so every following step can report
   failure somewhere observable).
2. Validate `BootInfo` (magic, version, size — mirrors what
   `tests/kernel_stub` already proved works from the bootloader side).
3. Memory manager bring-up: parse the UEFI memory map from `BootInfo`,
   initialize the physical frame allocator, then the kernel heap
   allocator (see MEMORY_MANAGER_DESIGN.md) — needed before anything
   past this point that allocates.
4. GDT/IDT and exception handling (next kernel subsystem after the
   memory manager; not implemented by the skeleton this ADR
   accompanies).
5. Timer + scheduler bring-up (after interrupts exist, since
   preemptive scheduling needs a timer interrupt).
6. Syscall entry point installation.

The skeleton implemented alongside this ADR performs only steps 1-2
and then halts — proving the Rust toolchain, the linker script, and
the `BootInfo` ABI all work correctly end to end, before any subsystem
with real design complexity (memory manager, scheduler) is built on
top of an unverified foundation.

### Early debug output vs. Phase 4 drivers
`arch/x86_64/serial.rs` is a minimal, kernel-owned COM1 UART driver
used only for early boot diagnostics — the same rationale as
`boot/serial.c` (ADR-004), reimplemented independently in Rust rather
than linked from the bootloader's C code, since ADR-004 already
established that the kernel's serial handling is not shared with the
bootloader's. This is intentionally not the formal driver model
Phase 4 will define (the C-ABI driver vtable interface ADR-001
anticipates) — it exists only so kernel code has some way to report
its own state before a real driver subsystem exists, exactly as the
bootloader needed one before any kernel existed at all.

### Panic policy
`panic = "abort"` (set in `Cargo.toml`, required anyway since there is
no unwinding runtime in a `no_std` freestanding binary). The panic
handler prints the panic message and location over the early debug
serial output, then halts via `hlt` in an infinite loop — there is no
recovery model yet (no process isolation exists this early), so a
kernel panic is unconditionally fatal for now. This will be revisited
once the scheduler and process model exist and "kill the offending
task instead of halting the machine" becomes a meaningful option.

### Coding standards
Full rules in `docs/kernel/CODING_STANDARDS.md`; referenced here
because they are a real architectural constraint (they govern how
`unsafe` is used at every hardware-facing boundary the memory manager
and scheduler will both need), not just a style preference.

## Consequences
- Every kernel subsystem from here on is a Rust module added to
  `kernel/src/`, compiled as one binary with the drivers that exist by
  Phase 4 — there is no per-subsystem process boundary to design
  around, simplifying (compared to a microkernel) IPC and scheduling
  but concentrating correctness requirements on `unsafe` code review,
  per the coding standards.
- The memory manager and scheduler designs (separate documents) both
  assume this single-address-space, ring-0-drivers model; if that ever
  changed, both documents would need to be revisited, not just this
  ADR.

## Alternatives Considered
- **Microkernel** (drivers as user-space servers, IPC-based): rejected
  — ADR-001 already committed to drivers-in-kernel-image via a C-ABI
  boundary, which is incompatible with a microkernel's process
  isolation for drivers. Revisiting this now would mean reopening a
  frozen ADR without a documented technical failure driving it, which
  the project's own rules don't permit.
- **Hybrid (some drivers in kernel, some as servers):** rejected for
  the same reason — no documented failure of the monolithic approach
  exists to justify the added complexity.
