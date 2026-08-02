/*
 * kernel_stub.c — Phase 2 Part 4 test fixture. See README.md in this
 * directory for why this exists and why it is NOT Phase 3 work.
 *
 * KernelEntry is reached via boot/trampoline.asm's `jmp rdx`, with
 * RDI holding a physical pointer to a populated BOOT_INFO struct
 * (System V AMD64 ABI — see ADR-005), running on the dedicated stack
 * and page tables the bootloader built for it.
 */

#include "../../boot/include/boot_info.h"
#include "../../boot/include/serial.h"

void KernelEntry(BOOT_INFO *info)
{
    /* A kernel should not assume a bootloader's device initialization
     * persists correctly into its own lifetime — it owns its own
     * drivers from the moment it starts. Re-initializing here, even
     * though the bootloader already did so moments ago, is correct
     * kernel practice, not redundant defensiveness. */
    SerialInit();

    SerialWriteString("\r\n================================================\r\n");
    SerialWriteString("XyronOS Kernel Stub - Phase 2 Part 4 test fixture\r\n");
    SerialWriteString("================================================\r\n");

    /* Validate the handoff before trusting anything else in it — see
     * boot_info.h and ADR-005 for why Magic/Version exist. A mismatch
     * here means either a corrupt handoff or a kernel/bootloader
     * built against different BootInfo layouts; either way, nothing
     * past this point can be trusted. */
    if (info->Magic != BOOTINFO_MAGIC) {
        SerialWriteString("FATAL: BootInfo magic mismatch - refusing to continue.\r\n");
        goto haltLoop;
    }
    if (info->Version != BOOTINFO_VERSION) {
        SerialWriteString("FATAL: BootInfo version mismatch - refusing to continue.\r\n");
        goto haltLoop;
    }
    if (info->StructSizeBytes != sizeof(BOOT_INFO)) {
        SerialWriteString("FATAL: BootInfo size mismatch - refusing to continue.\r\n");
        goto haltLoop;
    }
    SerialWriteString("[OK] BootInfo magic, version, and size validated.\r\n\r\n");

    SerialWriteString("Kernel physical base   : 0x");
    SerialWriteHex64(info->KernelPhysicalBase);
    SerialWriteString("\r\nKernel virtual base    : 0x");
    SerialWriteHex64(info->KernelVirtualBase);
    SerialWriteString("\r\nKernel size (bytes)    : 0x");
    SerialWriteHex64(info->KernelSizeBytes);
    SerialWriteString("\r\nKernel stack top       : 0x");
    SerialWriteHex64(info->KernelStackTop);
    SerialWriteString("\r\nKernel stack size      : 0x");
    SerialWriteHex64(info->KernelStackSizeBytes);
    SerialWriteString("\r\nMemory map entries     : 0x");
    SerialWriteHex64(info->MemoryMapEntryCount);
    SerialWriteString("\r\nMemory map desc. size  : 0x");
    SerialWriteHex64(info->MemoryMapDescriptorSize);
    SerialWriteString("\r\n\r\n");

    SerialWriteString("PHASE 2 COMPLETE: kernel entry reached with valid BootInfo.\r\n");
    SerialWriteString("Halting (this is a Phase 2 Part 4 test fixture, not the real kernel).\r\n");

haltLoop:
    for (;;) {
        __asm__ __volatile__("hlt");
    }
}
