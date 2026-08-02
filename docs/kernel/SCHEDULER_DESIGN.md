# Kernel Scheduler — Design

Status: **Designed, not yet implemented.** This document describes the
design a future kernel subsystem part will build. It depends on
subsystems that come before it in ADR-006's initialization order
(memory manager for task allocation, interrupts/timer for preemption)
and is not part of the skeleton accompanying ADR-006.

## Goals
1. Represent kernel and (eventually) user tasks uniformly enough that
   the same scheduler code handles both, without assuming a process
   model more complex than Phase 3 needs yet (no user-mode processes
   exist until later phases give userland something to run).
2. Preempt a running task on a timer interrupt and switch to another
   ready task, saving and restoring exactly the CPU state that
   matters, no more and no less.
3. Stay simple enough to reason about completely — this is the first
   scheduler this project has ever written; a simple, correct
   algorithm now is more valuable than a sophisticated one implemented
   incorrectly.

## Task model
```rust
struct Task {
    id: TaskId,
    state: TaskState,          // Ready, Running, Blocked, Terminated
    context: CpuContext,       // saved registers — see "Context switch" below
    kernel_stack: VirtAddr,    // top of this task's own kernel stack,
                                // allocated via the memory manager
    // Extended later (Phase 3 syscalls, later phases' process model)
    // with an address space handle once user-mode tasks exist —
    // omitted now since no code path creates one yet.
}
```
`TaskId` is a simple monotonically increasing integer, assigned at
task creation. No priority field yet — see "Scheduling algorithm"
below for why.

## Context switch
`CpuContext` holds exactly the callee-saved registers per the System V
AMD64 ABI (RBX, RBP, R12-R15, RSP) plus RIP (return address) — the
minimum needed to resume a task exactly where it left off. This
deliberately does NOT save caller-saved registers (RAX, RCX, RDX,
RSI, RDI, R8-R11) or floating-point/SSE state:
- Caller-saved registers are, by the calling convention itself, not
  expected to survive a function call — and a context switch is
  implemented as a function call into the scheduler, so the compiler
  has already ensured the caller-saved registers hold nothing the
  caller still needs by the time that call is made.
- No floating-point/SSE state exists to save yet — the kernel target
  spec (`toolchain/x86_64-os.json`) disables SSE (`-sse` feature) and
  uses soft-float, exactly so this doesn't need solving in the first
  scheduler implementation. Revisiting this is required before any
  task is allowed to use floating point, tracked as a known follow-up
  when that need arises, not implemented speculatively now.

The actual switch is a small hand-written assembly routine (same
justification as `boot/trampoline.asm`: a compiler cannot be trusted
to leave exactly the expected register state at a boundary this
precise) that saves the current task's `CpuContext` onto its kernel
stack, switches `RSP` to the next task's saved stack pointer, restores
that task's `CpuContext`, and returns — "returning" into whatever
instruction address was saved as that task's RIP, which is how a
previously-preempted task resumes exactly where it left off.

## Scheduling algorithm: round-robin
A single ready queue (`VecDeque<TaskId>`, from `alloc`, once the
kernel heap exists), no priority levels. On each timer interrupt: move
the current task to the back of the ready queue (if still `Ready`,
i.e. not blocked or terminated), pop the front of the queue, context
switch to it. Every ready task gets an equal, fixed time slice.

Round-robin, not a priority or multi-level feedback queue, is the
deliberate starting point: correctness of the context-switch mechanism
itself is the actual hard problem in a first scheduler, and a
priority scheme adds a second axis of complexity (priority inversion,
starvation) on top of an unproven context-switch path. Priority
scheduling is a natural, isolated follow-up once round-robin is proven
correct — not a redesign, an addition — and is explicitly deferred
rather than attempted now.

## Blocking and waking
`Task::state` transitions `Running -> Blocked` when a task waits on
something not yet ready (nothing in Phase 3 blocks yet — no I/O, no
IPC — this is designed now so the state machine doesn't need
retrofitting when Phase 3's syscall subsystem or Phase 4's drivers
introduce the first real blocking operation). A blocked task is
removed from the ready queue; waking it (whatever wakes it — a driver
interrupt handler in Phase 4, a syscall return in this phase) sets
`state = Ready` and re-enqueues it.

## What this document does not cover
- Multi-core scheduling (SMP) — this design assumes a single CPU core
  is running the scheduler; no locking strategy for a shared ready
  queue is designed here because none is needed yet. Revisited if/when
  SMP bring-up becomes a planned phase.
- User-mode process creation/`fork`-equivalent semantics — no
  `Cargo`-compiled userland program exists to run until much later
  phases; `Task` above is deliberately silent on address-space
  ownership until that's a real requirement.
- Real-time guarantees of any kind.
