/*
 * main.c — Bootloader entry point, Phase 2 Part 4 (final part of
 * Phase 2).
 *
 * Parts 1-3 (unchanged below) proved the toolchain, the file-I/O
 * pipeline, and correct ExitBootServices sequencing. Part 4 adds the
 * last piece: load a real kernel ELF image, build the page tables it
 * needs to run at its linked higher-half address, and jump to it —
 * the point where this bootloader's job is finished and Phase 3's
 * kernel (represented here by tests/kernel_stub, a minimal fixture —
 * see that directory's README for why this is not Phase 3 work) takes
 * over.
 *
 * Full sequence, in order (see ADR-005 for why this order matters):
 *   1. (Part 2) Open the ESP volume, prove file I/O works.
 *   2. (Part 4) Load the kernel ELF into a capped-address physical
 *      location — still using Boot Services (AllocatePages).
 *   3. (Part 4) Allocate and partially populate BootInfo — still
 *      using Boot Services.
 *   4. (Part 4) Build the page tables the kernel will run under —
 *      still using Boot Services.
 *   5. (Part 3) Retrieve the final memory map and call
 *      ExitBootServices, with the required stale-key retry.
 *   6. (Part 4) Finish populating BootInfo with the final memory map
 *      (a plain memory write — no Boot Services involved).
 *   7. (Part 4) Switch to the new page tables and jump to the kernel
 *      entry point. Never returns.
 */

#include "include/efi_types.h"
#include "include/efi_tables.h"
#include "include/efi_boot_services.h"
#include "include/efi_loaded_image_protocol.h"
#include "include/efi_file_protocol.h"
#include "include/memory_map.h"
#include "include/serial.h"
#include "include/boot_defs.h"
#include "include/boot_info.h"
#include "include/kernel_loader.h"
#include "include/paging.h"

/* Implemented in trampoline.asm — see that file and ADR-005 for the
 * calling-convention boundary this crosses. */
extern void JumpToKernel(UINT64 NewCr3, UINT64 KernelEntryVirtual,
                          UINT64 BootInfoPhysAddr, UINT64 KernelStackTop)
    __attribute__((noreturn));

static void PrintAscii(EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *ConOut,
                        CHAR8 *buffer, UINTN length)
{
    CHAR16 chunk[128];
    UINTN chunkLen = 0;

    for (UINTN i = 0; i < length; i++) {
        if (buffer[i] == '\n' && (chunkLen == 0 || chunk[chunkLen - 1] != L'\r')) {
            chunk[chunkLen++] = L'\r';
        }
        chunk[chunkLen++] = (CHAR16)buffer[i];
        if (chunkLen >= 125 || i == length - 1) {
            chunk[chunkLen] = 0;
            ConOut->OutputString(ConOut, chunk);
            chunkLen = 0;
        }
    }
}

EFI_STATUS EFIAPI EfiMain(EFI_HANDLE ImageHandle, EFI_SYSTEM_TABLE *SystemTable)
{
    EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *ConOut = SystemTable->ConOut;
    EFI_BOOT_SERVICES *BS = SystemTable->BootServices;
    EFI_STATUS status;
    BOOLEAN part2Success = FALSE;

    ConOut->ClearScreen(ConOut);
    ConOut->OutputString(ConOut, L"XyronOS Bootloader \x2014 Phase 2, Part 4\r\n");
    ConOut->OutputString(ConOut, L"Testing Simple File System read pipeline...\r\n\r\n");

    /* ===================== Part 2 (unchanged) ===================== */

    EFI_GUID loadedImageGuid = EFI_LOADED_IMAGE_PROTOCOL_GUID;
    EFI_LOADED_IMAGE_PROTOCOL *loadedImage = 0;

    status = BS->HandleProtocol(ImageHandle, &loadedImageGuid, (VOID **)&loadedImage);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: could not get LoadedImageProtocol\r\n");
        goto halt;
    }
    ConOut->OutputString(ConOut, L"[OK] LoadedImageProtocol acquired.\r\n");

    EFI_GUID sfspGuid = EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID;
    EFI_SIMPLE_FILE_SYSTEM_PROTOCOL *sfsp = 0;

    status = BS->HandleProtocol(loadedImage->DeviceHandle, &sfspGuid, (VOID **)&sfsp);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: could not get SimpleFileSystemProtocol\r\n");
        goto halt;
    }
    ConOut->OutputString(ConOut, L"[OK] SimpleFileSystemProtocol acquired.\r\n");

    EFI_FILE_PROTOCOL *root = 0;
    status = sfsp->OpenVolume(sfsp, &root);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: OpenVolume failed\r\n");
        goto halt;
    }
    ConOut->OutputString(ConOut, L"[OK] Root directory opened.\r\n");

    EFI_FILE_PROTOCOL *testFile = 0;
    status = root->Open(root, &testFile, L"\\BOOTINFO.TXT",
                         EFI_FILE_MODE_READ, 0);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: could not open \\BOOTINFO.TXT\r\n");
        ConOut->OutputString(ConOut, L"(Did you copy boot/testdata/BOOTINFO.TXT to the ESP root? See README.)\r\n");
        goto halt;
    }
    ConOut->OutputString(ConOut, L"[OK] \\BOOTINFO.TXT opened.\r\n");

    EFI_GUID fileInfoGuid = EFI_FILE_INFO_ID;
    UINTN infoSize = 0;
    testFile->GetInfo(testFile, &fileInfoGuid, &infoSize, 0);

    EFI_FILE_INFO *fileInfo = 0;
    status = BS->AllocatePool(EfiLoaderData, infoSize, (VOID **)&fileInfo);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: AllocatePool for file info failed\r\n");
        goto closeFile;
    }

    status = testFile->GetInfo(testFile, &fileInfoGuid, &infoSize, fileInfo);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: GetInfo (second call) failed\r\n");
        goto freeInfo;
    }
    ConOut->OutputString(ConOut, L"[OK] File size obtained.\r\n");

    UINTN fileDataSize = (UINTN)fileInfo->FileSize;
    CHAR8 *fileData = 0;

    status = BS->AllocatePool(EfiLoaderData, fileDataSize, (VOID **)&fileData);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: AllocatePool for file data failed\r\n");
        goto freeInfo;
    }

    status = testFile->Read(testFile, &fileDataSize, fileData);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: Read failed\r\n");
        goto freeData;
    }
    ConOut->OutputString(ConOut, L"[OK] File read into memory. Contents:\r\n");
    ConOut->OutputString(ConOut, L"------------------------------------------------------------\r\n");

    PrintAscii(ConOut, fileData, fileDataSize);

    ConOut->OutputString(ConOut, L"------------------------------------------------------------\r\n");
    ConOut->OutputString(ConOut, L"[OK] Part 2 file-read pipeline verified successfully.\r\n");
    part2Success = TRUE;

freeData:
    BS->FreePool(fileData);
freeInfo:
    BS->FreePool(fileInfo);
closeFile:
    testFile->Close(testFile);

    if (!part2Success) {
        goto halt;
    }

    /* ===================== Part 4: load the kernel =================== */

    ConOut->OutputString(ConOut, L"\r\nLoading kernel image \\KERNEL.ELF...\r\n");

    LOADED_KERNEL kernel;
    if (!LoadKernelElf(BS, root, L"\\KERNEL.ELF", &kernel)) {
        ConOut->OutputString(ConOut, L"FAILED: could not load or validate \\KERNEL.ELF\r\n");
        ConOut->OutputString(ConOut, L"(See tests/kernel_stub/README.md for how to build the test fixture.)\r\n");
        goto halt;
    }
    ConOut->OutputString(ConOut, L"[OK] Kernel image loaded and validated.\r\n");
    root->Close(root);

    /* ===================== Part 4: allocate + pre-populate BootInfo === */

    EFI_PHYSICAL_ADDRESS bootInfoPhys = IDENTITY_MAP_LIMIT - 1;
    status = BS->AllocatePages(AllocateMaxAddress, EfiLoaderData, 1, &bootInfoPhys);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: could not allocate BootInfo page\r\n");
        goto halt;
    }
    BOOT_INFO *bootInfo = (BOOT_INFO *)(UINTN)bootInfoPhys;
    for (UINTN i = 0; i < sizeof(BOOT_INFO); i++) {
        ((UINT8 *)bootInfo)[i] = 0;
    }
    bootInfo->Magic = BOOTINFO_MAGIC;
    bootInfo->Version = BOOTINFO_VERSION;
    bootInfo->StructSizeBytes = (UINT32)sizeof(BOOT_INFO);
    bootInfo->KernelPhysicalBase = kernel.KernelPhysicalBase;
    bootInfo->KernelVirtualBase = kernel.KernelVirtualBase;
    bootInfo->KernelSizeBytes = kernel.KernelSizeBytes;

    /* Allocate a dedicated kernel stack — 16 pages (64 KiB), ample for
     * a test fixture's needs and a reasonable starting point for an
     * early kernel before it sets up its own proper stack management
     * in Phase 3. Capped below IDENTITY_MAP_LIMIT for the same reason
     * as every other kernel-visible allocation (ADR-005): the stack
     * must be dereferenceable through the identity map the moment the
     * kernel starts using it, immediately after the CR3 switch. */
    #define KERNEL_STACK_PAGES 16
    EFI_PHYSICAL_ADDRESS stackPhys = IDENTITY_MAP_LIMIT - 1;
    status = BS->AllocatePages(AllocateMaxAddress, EfiLoaderData, KERNEL_STACK_PAGES, &stackPhys);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: could not allocate kernel stack\r\n");
        goto halt;
    }
    UINT64 stackSizeBytes = (UINT64)KERNEL_STACK_PAGES * PAGE_SIZE_4K;
    /* Stacks grow down on x86_64: the initial RSP must point to the
     * TOP (highest address) of the allocated region, not its base. */
    UINT64 stackTop = (UINT64)stackPhys + stackSizeBytes;
    bootInfo->KernelStackTop = stackTop;
    bootInfo->KernelStackSizeBytes = stackSizeBytes;
    ConOut->OutputString(ConOut, L"[OK] Kernel stack allocated.\r\n");
    ConOut->OutputString(ConOut, L"[OK] BootInfo allocated and pre-populated.\r\n");

    /* ===================== Part 4: build page tables =================== */

    UINT64 newCr3 = BuildKernelPageTables(BS, kernel.KernelPhysicalBase, kernel.KernelSizeBytes);
    if (newCr3 == 0) {
        ConOut->OutputString(ConOut, L"FAILED: could not build kernel page tables\r\n");
        goto halt;
    }
    ConOut->OutputString(ConOut, L"[OK] Page tables built (identity map + higher-half kernel mapping).\r\n");

    /* ===================== Part 3: memory map + exit ==================== */

    ConOut->OutputString(ConOut, L"\r\nPreparing to retrieve the memory map and exit Boot Services...\r\n");
    ConOut->OutputString(ConOut, L"(No further ConOut output is guaranteed valid after this point,\r\n");
    ConOut->OutputString(ConOut, L" per the UEFI specification. Remaining output goes to COM1.)\r\n");

    BOOT_MEMORY_MAP memMap;
    BOOLEAN exited = ExitBootServicesWithRetry(SystemTable, ImageHandle, &memMap);

    if (!exited) {
        ConOut->OutputString(ConOut, L"FAILED: ExitBootServices did not succeed within the retry budget.\r\n");
        goto halt;
    }

    /* -------------------------------------------------------------
     * Boot Services no longer exist. No EFI_BOOT_SERVICES function
     * may be called from here on. All further output uses raw COM1.
     * ------------------------------------------------------------- */

    bootInfo->MemoryMapPhysAddr = (UINT64)(UINTN)memMap.Map;
    bootInfo->MemoryMapSizeBytes = memMap.MapSizeBytes;
    bootInfo->MemoryMapDescriptorSize = memMap.DescriptorSize;
    bootInfo->MemoryMapDescriptorVersion = memMap.DescriptorVersion;
    bootInfo->MemoryMapEntryCount = memMap.EntryCount;

    SerialInit();
    SerialWriteString("\r\nXyronOS Bootloader - Phase 2 Part 4\r\n");
    SerialWriteString("ExitBootServices succeeded. Boot Services have been terminated.\r\n");
    SerialWriteString("BootInfo populated. Jumping to kernel entry point: 0x");
    SerialWriteHex64(kernel.EntryPointVirtual);
    SerialWriteString("\r\n");
    SerialWriteString("New CR3: 0x");
    SerialWriteHex64(newCr3);
    SerialWriteString("\r\n");
    SerialWriteString("This is the last line the bootloader itself prints.\r\n");
    SerialWriteString("Control transfers to the kernel now.\r\n\r\n");

    JumpToKernel(newCr3, kernel.EntryPointVirtual, (UINT64)(UINTN)bootInfo, stackTop);

    /* Unreachable — JumpToKernel is noreturn and its own assembly
     * contains no path back to this function. */

halt:
    ConOut->OutputString(ConOut, L"\r\nHalting.\r\n");
    for (;;) {
        __asm__ __volatile__("hlt");
    }

    return EFI_SUCCESS;
}
