# Kernel Interrupts and Exceptions — Design

Status: **Designed, not yet implemented as of this document's creation.**
This is the kernel subsystem ADR-006's "Boot flow and initialization
order" names as step 4, immediately after the Memory Manager subsystem
(step 3, complete as of v0.5.0) — see that ADR for why this order
matters and what step 5 (timer + scheduler) depends on this being done
first.

## Goals
1. Give the kernel its own GDT (Global Descriptor Table) — relying on
   whatever GDT UEFI firmware left active after `ExitBootServices` is
   not something this kernel controls or can rely on long-term, and
   the IDT setup below needs a known-stable code segment selector to
   reference.
2. Install an IDT (Interrupt Descriptor Table) covering all 32
   architecturally-reserved CPU exception vectors (0–31), so that any
   CPU exception occurring anywhere in the kernel from this point
   forward is caught and reported, rather than producing an
   unexplained triple fault and silent reset.
3. Report every caught exception through the existing serial debug
   path (`arch::x86_64::serial`, unchanged from the bootloader/memory
   manager subsystems) with enough detail (vector, error code where
   applicable, saved `RIP`/`CS`/`RFLAGS`/`RSP`/`SS`, and `CR2` for page
   faults) to actually diagnose what happened — not just "an exception
   occurred."
4. Do this without touching hardware interrupts (IRQs), the PIC/APIC,
   or `sti`/`cli` at all. CPU exceptions are not gated by `EFLAGS.IF`
   — only maskable hardware interrupts are — so full exception
   handling requires none of that. Timer interrupts (which DO need a
   configured interrupt controller and `sti`) are explicitly the next
   subsystem's job, per ADR-006's own ordering; this subsystem stops
   at the CPU's own architectural exceptions.

## GDT design
Three real segment descriptors plus one TSS descriptor (which occupies
two GDT slots in long mode, since a system-segment descriptor is
16 bytes instead of 8):

| Index | Selector | Purpose |
|---|---|---|
| 0 | `0x00` | Null descriptor (architecturally required) |
| 1 | `0x08` | Kernel code segment (64-bit, ring 0) |
| 2 | `0x10` | Kernel data segment (ring 0) |
| 3–4 | `0x18` | TSS (Task State Segment) descriptor |

No user-mode (ring 3) segments exist yet — nothing runs at ring 3
until a process model exists (`docs/kernel/SCHEDULER_DESIGN.md`, not
this subsystem). Adding them now would be exactly the kind of
speculative, not-yet-needed code this project's rules ask to avoid.

In 64-bit long mode, segment *base* and *limit* are ignored by the CPU
for code/data access (flat memory model is architecturally forced) —
the code/data descriptors here exist only to carry the access-rights
and mode bits (present, ring level, executable, the long-mode `L` bit)
the CPU still checks on every segment-register load and every
interrupt/exception entry. This is why the descriptors can be `const`
values computed once, not something requiring runtime configuration
based on detected memory size the way the bootloader's own page tables
needed to be.

**Why a TSS, with no ring-3 code to switch privilege levels for yet:**
a TSS in long mode is not primarily about privilege-level stack
switching anymore (that still exists, but isn't needed here) — it also
holds the **Interrupt Stack Table (IST)**, up to seven alternate stack
pointers any IDT entry can be told to switch to unconditionally on
entry, regardless of the current privilege level. This exists
specifically for exceptions that must never run on a stack that might
itself already be the problem — the canonical example, and the only
one this subsystem uses it for, is **double fault**: if a kernel stack
overflow is what caused a fault, the double-fault handler running on
that same, already-exhausted stack would immediately fault again,
producing an unrecoverable triple fault with no diagnostic output at
all. Running it on a small, dedicated, always-valid IST stack instead
means the handler can reliably run and report *something* even in that
scenario.

## IDT design
All 256 possible vectors exist in the table's storage (a fixed-size
`[IdtEntry; 256]` array, per the architecture's own table format), but
only vectors 0–31 (the architecturally-defined exceptions) are
installed with real handlers this subsystem. Vectors 32–255 (where
hardware IRQs and any future software interrupts would live) are left
present-bit-clear — an interrupt arriving on one of those before the
next subsystem installs a real handler is itself something worth
knowing about, and a clear/absent entry causes a well-defined
"Interrupt does not exist" fault from the CPU rather than jumping to
uninitialized memory, which a zeroed-but-marked-present entry would
risk.

Each of the 32 exception vectors has a distinct, architecturally-fixed
meaning and — critically — a distinct **stack layout on entry**: some
push a 64-bit error code before the standard interrupt frame
(`RIP`/`CS`/`RFLAGS`/`RSP`/`SS`), most do not. Getting this wrong for
even one vector misreads every field after it. The exact list (from
the Intel SDM Vol. 3A / AMD64 APM Vol. 2 exception reference — an
external hardware specification, implemented independently, the same
category as every other CPU/firmware structure this project has
implemented so far):

| Vector | Name | Mnemonic | Error code? |
|---|---|---|---|
| 0 | Divide Error | #DE | No |
| 1 | Debug | #DB | No |
| 2 | Non-Maskable Interrupt | — | No |
| 3 | Breakpoint | #BP | No |
| 4 | Overflow | #OF | No |
| 5 | Bound Range Exceeded | #BR | No |
| 6 | Invalid Opcode | #UD | No |
| 7 | Device Not Available | #NM | No |
| 8 | Double Fault | #DF | Yes (always 0) |
| 9 | Coprocessor Segment Overrun (legacy, reserved) | — | No |
| 10 | Invalid TSS | #TS | Yes |
| 11 | Segment Not Present | #NP | Yes |
| 12 | Stack-Segment Fault | #SS | Yes |
| 13 | General Protection Fault | #GP | Yes |
| 14 | Page Fault | #PF | Yes |
| 15 | Reserved | — | No |
| 16 | x87 Floating-Point Exception | #MF | No |
| 17 | Alignment Check | #AC | Yes |
| 18 | Machine Check | #MC | No |
| 19 | SIMD Floating-Point Exception | #XM | No |
| 20 | Virtualization Exception | #VE | No |
| 21 | Control Protection Exception | #CP | Yes |
| 22–27 | Reserved | — | No |
| 28 | Hypervisor Injection Exception | #HV | No |
| 29 | VMM Communication Exception | #VC | Yes |
| 30 | Security Exception | #SX | Yes |
| 31 | Reserved | — | No |

Handler functions use Rust's `extern "x86-interrupt"` calling
convention (an unstable, nightly-only ABI — already acceptable in this
codebase, which builds via `RUSTC_BOOTSTRAP=1`/nightly throughout, per
`kernel/README.md`), which generates the correct prologue/epilogue
(saving/restoring the caller-saved registers the CPU doesn't save
automatically, and using `iretq` rather than `ret` to return) for
exactly this purpose. Two distinct function signatures are needed —
one for the "no error code" vectors, one for the "has error code"
vectors — matching the table above exactly; using the wrong signature
for a given vector is exactly the class of bug this design table
exists to prevent.

**32 near-identical handler functions, generated by a macro, not
hand-duplicated.** Each vector's handler must know its own vector
number at compile time (the CPU does not push the vector number for
CPU exceptions the way it can for software `int n` — the handler has
no other way to know which vector invoked it), so a `macro_rules!`
generates one small `extern "x86-interrupt"` function per vector, each
hardcoding its own vector number in a call to a single shared
diagnostic-reporting function. This is boilerplate generation, not a
shortcut — every one of the 32 generated functions is a real,
distinct, correctly-typed function with its own IDT entry; nothing
about them is a placeholder.

## Diagnostic reporting
Every handler (fatal or not) reports, over the existing serial debug
path:
- The vector number and its name (from the table above).
- The error code, decoded per-vector where the architecture defines
  specific bit meanings (page fault's error code encodes
  present/write/user/reserved-write/instruction-fetch as individual
  bits, for one) — not just the raw hex value.
- `CR2` (the faulting virtual address), for page faults specifically —
  the architecture only guarantees `CR2` is valid immediately following
  a page fault, so this must be read inside the handler, before
  anything else that could itself fault or otherwise disturb it.
- The saved `RIP`, `CS`, `RFLAGS`, `RSP`, `SS` from the interrupt
  stack frame the CPU pushed automatically.

## Fault severity and the panic policy
Per ADR-006's existing panic policy (unconditional halt — no process
isolation exists yet to make anything less drastic meaningful): every
one of the 32 handlers halts after reporting, **except** breakpoint
(`#BP`, vector 3), which is architecturally meant to be a debugging
aid that returns control to right after the triggering instruction —
its handler reports and returns normally. This is not a special case
invented for convenience; it is what `#BP` is *for*, and it doubles as
this subsystem's own proof that the full entry → handler → `iretq`
→ resume path works correctly, not just entry.

## Initialization order
Within step 4 of ADR-006's boot flow:
1. Build and load the GDT (`lgdt`), including the TSS descriptor and
   the TSS itself (with its one configured IST entry for double
   fault).
2. Reload every segment register (`CS` via the long-mode
   far-return trick — there is no direct way to write `CS` — `DS`/
   `ES`/`SS`/`FS`/`GS` via plain `mov`) to point at the new GDT's
   selectors. Until this happens, the CPU is still technically running
   under whatever segment state UEFI firmware left active, even though
   the new GDT is loaded.
3. Load the TSS itself (`ltr`) — a separate instruction from loading
   the GDT; the GDT only describes where the TSS *is*, `ltr` tells the
   CPU to actually start using it (this is what makes IST entries
   effective).
4. Build and load the IDT (`lidt`), with all 32 exception vectors
   installed pointing at their generated handlers, IDT entry 8
   (double fault) specifically configured to use IST index 1.
5. Boot self-test (see below) — the last step before this subsystem
   reports itself complete.

## Testing strategy
No part of this subsystem is meaningfully host-testable the way
`mm/`'s pure-logic pieces were: GDT/IDT/TSS setup is inherently about
real CPU privileged state (`lgdt`/`lidt`/`ltr`, segment registers,
`CR2`) that has no meaningful standalone behavior on a host machine
running as an ordinary userspace test process. This subsystem is
therefore verified entirely via a boot-time integration self-test,
matching the precedent every hardware-facing piece of `mm/` already
established (`FrameAllocator::init`, `VirtualMemoryManager`'s
`map`/`unmap`, `KernelHeap`'s growth) — not a gap, a continuation of
the same, already-established testing split.

The self-test deliberately triggers two real exceptions, not just
"loads the IDT and claims success":
1. **`int3`** (breakpoint, vector 3, no error code) — proves entry,
   correct diagnostic reporting, and `iretq`-based return-and-resume
   all work; execution continues normally afterward, allowing further
   checks in the same boot.
2. **A genuine page fault, triggered by writing to a page deliberately
   mapped read-only** (vector 14, has an error code, requires `CR2`)
   — deliberately triggered as the LAST act of this subsystem's
   self-test, since its handler halts per the panic policy above.
   This specific trigger was chosen over a simpler "dereference an
   unmapped address" fault because it does double duty: it proves the
   page-fault path works AND it closes a gap `docs/kernel/MEMORY_MANAGER_DESIGN.md`
   and `TODO.md` explicitly flagged as unverified when the virtual
   memory manager was built (v0.4.0) — that `PageFlags::writable =
   false` is not just stored correctly (already proven then, via
   `flags_at()`) but actually *enforced* by the CPU. The error code's
   decoded bits (present=1, write=1) and `CR2` (matching the exact
   address written to) are themselves the proof. The halt that follows
   is the expected, correct end state, not a failure.

**What this does not verify, stated rather than assumed:** the
double-fault IST stack switch is configured correctly (real code, not
a placeholder), but actually *exercising* it — deliberately causing a
kernel stack overflow to confirm the CPU really does switch to the
IST stack rather than faulting again on the overflowed one — is not
attempted this milestone. Doing so safely (without risking corrupting
other kernel state in a way that makes the rest of the self-test
unreliable) is a more invasive test than this subsystem's other
checks; recorded as a follow-up in `TODO.md` rather than skipped
silently.

## What this document does not cover
- Hardware interrupts (IRQs), the PIC/APIC, or `sti`/`cli` — the
  Timer subsystem, next per ADR-006's ordering.
- Any handler behavior beyond "report and halt" (or, for breakpoint,
  "report and resume") — recovery, killing an offending task instead
  of halting the machine, becomes meaningful once the scheduler's
  process model exists, not before.
- User-mode (ring 3) segments or privilege-level transitions — no
  code will run at ring 3 until much later phases.
