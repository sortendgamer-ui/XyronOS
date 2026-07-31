# ADR-001: Kernel Implementation Language Split

## Status
Accepted — 2026-07-31

## Context
The kernel needs an implementation language for two very different kinds of
code:
1. Core subsystems where a single memory-safety bug (use-after-free, buffer
   overrun, data race) can corrupt kernel state and crash or compromise the
   entire machine: the scheduler, the memory manager, and inter-process
   communication (IPC).
2. Device drivers, which spend most of their code talking directly to
   memory-mapped hardware registers, follow vendor datasheets that are
   usually written against C calling conventions, and often need to be
   ported quickly from existing open reference code (rewritten from scratch
   per project rules, but the *shape* of the code — register structs, bit
   masks, MMIO reads/writes — maps far more directly from C).

## Decision
- **Kernel core** (scheduler, memory manager, IPC, capability system,
  syscall dispatch): written in **Rust**, `#![no_std]`, no heap allocation
  in the earliest boot path, custom global allocator introduced once the
  memory manager is online.
- **Device drivers**: written in **C11**, compiled freestanding
  (`-ffreestanding -fno-builtin -nostdlib`), linked into the kernel image
  through a well-defined C-ABI driver interface exposed by the Rust core.
- **Bootloader and lowest-level entry points** (real mode → protected mode →
  long mode transition, initial GDT/IDT/page tables): raw **x86_64 assembly
  (NASM)**, per ADR to be written in Phase 2.

## Consequences
- Every driver must expose a fixed C-ABI vtable (init, read, write, irq
  handler, shutdown) so the Rust core can call into it without depending on
  Rust ABI stability across the boundary. This vtable will be formalized in
  ADR-004 (Driver Model) during Phase 4.
- The Rust core must be built with a custom target spec
  (`x86_64-unknown-none`) and cannot use the Rust standard library — only
  `core` and `alloc` (once a global allocator exists).
- Two toolchains must be present in the build environment: a Rust
  cross-compiler targeting `x86_64-unknown-none`, and an `x86_64-elf-gcc`
  (or clang with `--target=x86_64-elf`) cross-compiler for the C driver
  code. Both link against the same linker script.
- Panics in the Rust core must be caught at the C-ABI boundary (no Rust
  panic may unwind into C code) — drivers calling into the core will see
  a documented error code, never a Rust panic.

## Alternatives Considered
- **Pure Rust, including drivers**: rejected — hardware register
  descriptions and MMIO patterns from datasheets are C-shaped, and forcing
  every driver through `unsafe` Rust wrappers earlier than necessary adds
  friction with no safety benefit until the driver model (ADR-004) exists
  to contain it properly.
- **Pure C**: rejected — the core subsystems are exactly the place where
  memory-safety bugs are most catastrophic (privilege level 0, no fault
  isolation), and Rust's ownership model removes whole classes of bugs
  there for free.
