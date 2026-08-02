; trampoline.asm — the actual bootloader-to-kernel handoff instruction
; sequence. See ADR-005 for the full design; this file is deliberately
; tiny — everything that can be validated or computed happens in C
; (kernel_loader.c, paging.c, main.c) BEFORE this function is ever
; called. By the time JumpToKernel runs, every decision has already
; been made; this code only executes the point-of-no-return switch.
;
; Why assembly and not C: the instant after `mov cr3, rcx` executes,
; the CPU is running under entirely new page tables, and the very next
; instruction fetch depends on that new mapping being correct. C gives
; no way to guarantee that the compiler hasn't inserted stack-protector
; checks, red-zone usage, or any other instruction between "set CR3"
; and "we're safely executing known-mapped code" — a single wrong
; assumption there is silent, undebuggable corruption. Hand-written
; assembly with a fixed, auditable instruction sequence is the only
; way to make this boundary provably correct, which is exactly the
; category of "lowest-level entry point" ADR-001 reserved for NASM.
;
; Calling convention: this function is called from main.c (compiled
; for the bootloader's native Microsoft x64 ABI — see ADR-005), so
; parameters arrive in RCX, RDX, R8, R9 per that ABI:
;   RCX = NewCr3            (physical address of the PML4 table)
;   RDX = KernelEntryVirtual (virtual address, valid only after the
;                             CR3 switch below activates the mapping
;                             paging.c built for it)
;   R8  = BootInfoPhysAddr  (physical address of the populated
;                             BOOT_INFO struct; also a valid virtual
;                             address post-switch, since it falls
;                             inside the identity-mapped region)
;   R9  = KernelStackTop    (initial RSP value for the kernel's own,
;                             dedicated stack — see boot_info.h for
;                             why we do not hand the kernel whatever
;                             stack UEFI happened to be using)
;
; Before jumping, R8 is moved into RDI — the kernel's own calling
; convention is System V AMD64 (RDI holds the first integer
; argument), matching what the Rust kernel core will expect by
; default once Phase 3 begins. This one instruction IS the entire
; ABI boundary between the two halves of the system.

section .text
global JumpToKernel

JumpToKernel:
    ; Activate the new page tables. From this instruction onward,
    ; every memory access — including the instruction fetch for the
    ; very next line — is translated through the mapping paging.c
    ; built: an identity map of low physical memory (so this code,
    ; still executing from wherever firmware loaded the bootloader,
    ; keeps working) plus the higher-half kernel mapping (so the jmp
    ; below lands somewhere real).
    mov cr3, rcx

    ; Switch to the kernel's own dedicated stack before doing
    ; anything else that could conceivably touch the stack (nothing
    ; below does, but this ordering keeps that guarantee obviously
    ; true by inspection rather than by accident).
    mov rsp, r9

    ; Set up the kernel's expected argument register (System V AMD64
    ; ABI: first integer argument in RDI) before transferring control.
    mov rdi, r8

    ; Transfer control. This is an indirect jump through a register,
    ; not a call — the bootloader is not a function the kernel will
    ; ever return into; there is deliberately no return address on
    ; the stack for it to return to, and no code here after the jump
    ; for it to return to even if it tried.
    jmp rdx

    ; Unreachable. No further bootloader code executes past this
    ; point under any circumstance the code above can produce.
