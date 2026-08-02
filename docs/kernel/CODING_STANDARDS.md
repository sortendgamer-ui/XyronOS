# Kernel Coding Standards

Applies to all Rust code under `kernel/`. Referenced from ADR-006 as a
real architectural constraint, not a style guide — most of these rules
exist specifically because kernel code runs with no memory protection
from itself and no OS underneath to catch its mistakes.

## `unsafe` usage
- Every `unsafe` block is preceded by a `// SAFETY:` comment explaining
  *why* the operation is actually sound at this call site — not what
  the code does (that should be obvious from reading it), but which
  invariant the surrounding code guarantees that makes the unsafe
  operation valid. "SAFETY: caller guarantees X" is acceptable only if
  the function itself is also `unsafe fn` and documents that it
  requires X of its callers.
- `unsafe` blocks are kept as small as possible — wrap only the
  specific operation that requires it (a raw pointer dereference, an
  `asm!` call, a register write), never a whole function body as a
  matter of convenience.
- Hardware-facing primitives (port I/O, control register access, page
  table writes) are wrapped in safe-signature functions wherever the
  wrapper can actually enforce the safety invariant (e.g. a `map()`
  function that takes already-validated `VirtAddr`/`PhysAddr` newtypes
  rather than raw `u64`) — pushing `unsafe` to the narrowest possible
  boundary rather than letting it leak into every call site.

## Panics and `Result`
- Kernel code that can fail in a way the caller can reasonably handle
  returns `Result`, not a panic — allocation failure, an invalid
  `BootInfo`, a page-table mapping conflict are all `Result` territory.
- `panic!` (including via `.unwrap()`/`.expect()`) is reserved for
  conditions that indicate a genuine kernel bug or corrupted state
  where continuing would be worse than stopping — per ADR-006's panic
  policy, this currently means halting the whole machine, so panicking
  is never the "convenient" choice for an error a caller could
  otherwise recover from.
- `.expect("message")` over bare `.unwrap()` everywhere a panic is the
  deliberate choice — the message is what appears in the panic output
  over serial, and "called unwrap on a None value" tells a future
  debugger nothing a bare panic wouldn't already say.

## No placeholder code
Matches the project-wide rule, restated for what it means specifically
in kernel code: a function is not committed until it does what its
signature promises. A `TODO`-and-`unimplemented!()` stub is not an
acceptable way to "reserve" an API — a module is added when the part
that implements it begins, not before, per ADR-006's module layout.

## Formatting and structure
- `rustfmt` default settings, no project-specific overrides — enforced
  in CI once the kernel has a `static-analysis.yml` job (added
  alongside the first kernel code that CI needs to lint; see that
  workflow file's history).
- `clippy` clean, `-D warnings` — same enforcement point as `rustfmt`.
- One `unsafe fn` or hardware-facing module per file where practical
  (e.g. `arch/x86_64/serial.rs` owns all COM1 port I/O) — keeps the
  set of code that needs the most careful review physically
  contained, rather than scattered across files that are mostly safe
  code.

## Comments
- Every module-level doc comment (`//!`) states what the module owns
  and, where relevant, what it deliberately does not do yet — matching
  this project's existing documentation style in `boot/` and the ADRs,
  not a new convention invented for the kernel.
- A comment explaining *why* accompanies any code whose correctness
  depends on something not visible in the code itself: an ABI
  requirement (register conventions, struct layout matching a C
  struct), a hardware behavior (why a delay or a specific I/O sequence
  is needed), or an ordering requirement between two operations that
  looks reorderable but isn't.

## Testing
`no_std` kernel code cannot use the standard `#[test]` harness (it
depends on the standard library and an OS to run under). Until Phase
17 (Testing) defines the project's real in-kernel or QEMU-driven test
strategy, correctness is established the same way Phase 2 established
it: build cleanly, boot in QEMU, and verify real, specific output —
never "it compiled" alone. Every kernel subsystem part's completion
criteria includes an actual boot test demonstrating the new code
working, matching the standard this project has used since Phase 2
Part 1.
