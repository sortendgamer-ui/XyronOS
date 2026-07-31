/*
 * main.c — Bootloader entry point, Phase 2 Part 1.
 *
 * This is intentionally the smallest possible program that proves the
 * entire chain works end to end: our own UEFI struct definitions match
 * real firmware's memory layout, our PE32+ build produces something
 * firmware accepts as a valid UEFI application, and our entry point
 * signature is correct. Every later part of Phase 2 (reading files off
 * disk, reading the memory map, exiting boot services, jumping into
 * the kernel) builds on this same skeleton, so it must be verified
 * working before anything else is added.
 *
 * What this program deliberately does NOT do yet:
 *   - Read anything from disk (Part 2)
 *   - Touch memory map / call ExitBootServices (Part 3)
 *   - Set up page tables or enter long mode (already in long mode —
 *     UEFI on x86_64 always runs in 64-bit mode; see note below)
 *   - Jump to a kernel (doesn't exist until Phase 3)
 *
 * Note on "long mode": x86_64 UEFI firmware is REQUIRED by the spec to
 * call our entry point already running in 64-bit long mode with paging
 * enabled (an identity-mapped or firmware-managed page table). The
 * classic "real mode -> protected mode -> long mode" transition that
 * BIOS bootloaders perform is firmware's job here, not ours — one of
 * the reasons ADR-001 chose UEFI-only boot.
 */

#include "include/efi_types.h"
#include "include/efi_tables.h"

/*
 * EfiMain — the UEFI application entry point.
 *
 * Signature is mandated by UEFI Spec Section 4.1 (EFI_IMAGE_ENTRY_POINT).
 * Firmware calls this exact signature; the linker is told (via the
 * Makefile) that this symbol is the PE entry point.
 */
EFI_STATUS EFIAPI EfiMain(EFI_HANDLE ImageHandle, EFI_SYSTEM_TABLE *SystemTable)
{
    /* ImageHandle identifies this loaded image to firmware services we
     * don't call yet (LoadImage/StartImage bookkeeping). Silence the
     * unused-parameter warning without discarding the parameter — we
     * will need it starting Part 2 when we call LoadImage for the
     * kernel, so it stays in the signature now rather than being added
     * later and disturbing an already-tested entry point. */
    (void)ImageHandle;

    /* ConOut is guaranteed non-NULL and ready to use the instant our
     * entry point runs — this is part of the UEFI Boot Services
     * contract (Spec Section 4.3). */
    EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *ConOut = SystemTable->ConOut;

    /* Clear the firmware's pre-boot splash/log text so our output is
     * unambiguous in a screenshot or serial log. */
    ConOut->ClearScreen(ConOut);

    /* UEFI console strings are CHAR16 (UCS-2). Under the mingw-w64
     * cross-compiler (ADR-001 toolchain), wchar_t is natively 16 bits,
     * so an L"" literal is already the correct width — no manual UTF-8
     * to UCS-2 conversion needed for this static string. */
    ConOut->OutputString(ConOut, L"OS Bootloader — Phase 2, Part 1\r\n");
    ConOut->OutputString(ConOut, L"UEFI entry point reached successfully.\r\n");
    ConOut->OutputString(ConOut, L"Struct layout and toolchain verified.\r\n");

    /* Part 1 has no further work: nothing has been loaded, no boot
     * services have been exited, so returning EFI_SUCCESS here would
     * hand control back to the firmware boot manager, which would then
     * likely retry booting or drop to its own menu. Since our purpose
     * right now is purely to prove we booted, we halt explicitly and
     * visibly instead of silently returning control we haven't built
     * a real continuation for yet. */
    ConOut->OutputString(ConOut, L"Halting (Part 1 has no further tasks).\r\n");

    for (;;) {
        /* HLT halts the processor until the next interrupt. This is a
         * real, correct way to idle — not a placeholder — and matches
         * what the kernel's own idle loop will do starting in Phase 3. */
        __asm__ __volatile__("hlt");
    }

    /* Unreachable, but every function must satisfy its declared return
     * type for the compiler's flow analysis. */
    return EFI_SUCCESS;
}
