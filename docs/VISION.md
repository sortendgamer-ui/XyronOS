# Project Vision & Feature Scope

## Vision
An original operating system, built component-by-component with no code
borrowed from Linux, Windows, macOS, or BSD. Where we must interoperate
with the outside world (CPU architecture, UEFI firmware, ACPI tables,
USB, PCIe), we implement those *specifications* independently from their
public documentation — never their reference implementations.

## Guiding principles
1. Correctness and clarity over cleverness — every subsystem has a written
   ADR before it has code.
2. Safety by construction where it's cheap (Rust core) — not bolted on
   after the fact.
3. No phase is "done" until it builds and boots/runs under QEMU.

## v1 feature scope (what "Stable Release" in Phase 20 means)
- Boots on UEFI firmware, x86_64, from a USB drive or QEMU virtual disk.
- Preemptive multitasking kernel with a real scheduler, virtual memory,
  and a syscall ABI.
- Drivers for: keyboard, mouse, a basic GPU framebuffer, a SATA/NVMe disk,
  and one common NIC (e.g. Intel e1000, since it's well-documented and
  widely emulated by QEMU for testing).
- A journaling filesystem of our own design.
- A minimal but functional TCP/IP stack (Ethernet → IP → TCP/UDP → sockets
  API) enough to fetch a URL over HTTP.
- A compositing window manager and a small desktop shell.
- A handful of built-in apps: terminal, text editor, file manager, browser
  (basic HTML/HTTP renderer — not a full web engine).
- A package manager for installing additional software.
- A self-hosting-capable compiler toolchain for at least one language we
  define, sufficient to build simple userland programs.
- An integrated AI assistant service with OS-level hooks (system search,
  automation, natural-language shell).
- Basic gaming API (2D/3D acceleration hooks against our graphics stack).
- Baseline security model: process isolation, capability-based
  permissions, signed packages.

## Explicitly out of scope for v1
- Multi-architecture support beyond x86_64 (AArch64 considered post-v1).
- Full POSIX compatibility layer (not a goal — this is not a Linux clone).
- Legacy BIOS boot support.
