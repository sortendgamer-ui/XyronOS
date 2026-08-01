# ADR-004: Post-ExitBootServices Diagnostic Output via Raw 16550 UART

## Status
Accepted — 2026-07-31

## Context
Once `ExitBootServices` succeeds, the UEFI specification no longer
guarantees that any Boot Services — including
`EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL->OutputString` (`ConOut`), which is
associated with Boot Services on most implementations — are safe to
call. Part 3 needs to confirm, from within the bootloader itself, that
`ExitBootServices` actually succeeded and that execution continues
correctly afterward. No graphics/framebuffer driver exists yet (that
is Phase 7, and even a basic GOP-framebuffer bootloader print routine
is out of scope until this bootloader needs to hand a framebuffer
pointer to the kernel in a later part). Some diagnostic channel that
does not depend on firmware services is needed now.

## Decision
Implement a minimal, real 16550-compatible UART driver
(`boot/serial.c`) that talks directly to COM1 (I/O port `0x3F8`) via
raw `in`/`out` port instructions — no firmware calls, no BIOS
"Int 10h"-style services (which do not exist in UEFI's runtime model
anyway), no dependency on firmware having configured the port already.
The driver explicitly programs the baud rate divisor, line control
register (8 data bits, no parity, 1 stop bit), and enables the FIFO,
rather than assuming firmware left the port in a usable state.

This becomes the bootloader's only output channel after
`ExitBootServices` succeeds. Before that point, `ConOut` remains the
primary output channel (unchanged from Parts 1-2) since it is simpler
and firmware-guaranteed valid up to the moment of a successful exit.

## Consequences
- The bootloader now contains its first genuine hardware driver, ahead
  of Phase 4's formal driver work — this is scoped narrowly (COM1 UART
  output only, no input, no interrupt-driven I/O) and exists purely for
  bootloader-internal diagnostics.
- This UART driver is bootloader-local, not shared with the kernel's
  eventual driver model (ADR to be written when Phase 4 defines the
  driver vtable interface) — the kernel will have its own serial driver
  under `kernel/drivers/`, written independently, since a bootloader
  component should not become a dependency the kernel links against.
- On real hardware without a COM1 UART present (increasingly common on
  modern laptops), writes to port `0x3F8` are silently discarded by the
  system's I/O subsystem in the typical case — this is diagnostic-only
  output, not required for correct boot behavior, so this is an
  acceptable limitation, not a defect. The bootloader's actual boot
  logic does not depend on the UART being present or working.

## Alternatives Considered
- **Rely on ConOut after ExitBootServices anyway:** rejected — this
  works in QEMU/OVMF (and many real implementations) as an
  implementation detail, but the spec explicitly does not guarantee it,
  and this project's rule against shortcuts means not depending on
  unspecified behavior even where it happens to work today.
- **Write directly to a VGA text-mode buffer (0xB8000):** rejected —
  UEFI does not guarantee a VGA-compatible text buffer exists at that
  physical address; GOP (Graphics Output Protocol) framebuffers, which
  UEFI does guarantee when a GPU driver is present, are pixel
  framebuffers, not VGA text mode. Text-buffer output would be a
  legacy-BIOS-era assumption inconsistent with ADR-001's UEFI-only
  decision.
- **Defer all post-exit confirmation to Phase 4/7 drivers:** rejected —
  Part 3's own success criteria (confirm ExitBootServices actually
  worked) would then be unverifiable within Phase 2, violating the
  project rule that every phase's stated success criteria must be met
  before moving on.
