/*
 * main.c — Bootloader entry point, Phase 2 Part 3.
 *
 * Parts 1-2 (unchanged below) proved the toolchain/struct layout and
 * the file-I/O pipeline. Part 3 adds the single most consequential
 * step a UEFI bootloader ever takes: retrieving the final memory map
 * and calling ExitBootServices, the point of no return after which
 * firmware's own services (including, in general, ConOut) can no
 * longer be relied upon. See memory_map.c for why this requires a
 * retry loop rather than one call, and ADR-004 for why a raw serial
 * driver exists to confirm success afterward.
 *
 * What this program deliberately does NOT do yet:
 *   - Load an actual kernel or jump to it (Part 4 — no kernel exists
 *     until Phase 3 begins). The memory map captured here is exactly
 *     what Part 4 will hand off to the kernel once one exists.
 */

#include "include/efi_types.h"
#include "include/efi_tables.h"
#include "include/efi_boot_services.h"
#include "include/efi_loaded_image_protocol.h"
#include "include/efi_file_protocol.h"
#include "include/memory_map.h"
#include "include/serial.h"

/*
 * PrintAscii — convert and print a narrow (CHAR8/ASCII) buffer through
 * the UEFI console, which only accepts CHAR16 (UCS-2) strings.
 * Unchanged from Part 2.
 */
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
    ConOut->OutputString(ConOut, L"XyronOS Bootloader \x2014 Phase 2, Part 3\r\n");
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

    /* ===================== Part 3 (new) ============================
     * Only reached if Part 2 fully succeeded — an early file-I/O
     * failure has no bearing on whether memory-map/exit logic works,
     * so there is no reason to exercise it on a run that already
     * failed for an unrelated reason. */

    ConOut->OutputString(ConOut, L"\r\nXyronOS Bootloader \x2014 Phase 2, Part 3\r\n");
    ConOut->OutputString(ConOut, L"Preparing to retrieve the memory map and exit Boot Services...\r\n");
    ConOut->OutputString(ConOut, L"(No further ConOut output is guaranteed valid after this point,\r\n");
    ConOut->OutputString(ConOut, L" per the UEFI specification. Remaining output goes to COM1.)\r\n");

    BOOT_MEMORY_MAP memMap;
    BOOLEAN exited = ExitBootServicesWithRetry(SystemTable, ImageHandle, &memMap);

    if (!exited) {
        /* Boot Services are still active in this branch (we never
         * reached a successful ExitBootServices call), so ConOut is
         * still safe to use here. */
        ConOut->OutputString(ConOut, L"FAILED: ExitBootServices did not succeed within the retry budget.\r\n");
        goto halt;
    }

    /* -------------------------------------------------------------
     * Boot Services no longer exist. From this line onward, no
     * EFI_BOOT_SERVICES function — including anything reached through
     * ConOut — may be called. All further output uses the raw COM1
     * driver from serial.c/serial.h (see ADR-004).
     * ------------------------------------------------------------- */

    SerialInit();
    SerialWriteString("\r\nXyronOS Bootloader - Phase 2 Part 3\r\n");
    SerialWriteString("ExitBootServices succeeded. Boot Services have been terminated.\r\n");
    SerialWriteString("This message was written directly to COM1 (I/O port 0x3F8),\r\n");
    SerialWriteString("with no firmware services involved, proving the exit was real.\r\n\r\n");

    SerialWriteString("Final memory map:\r\n");
    SerialWriteString("  Entry count      : ");
    SerialWriteHex64((UINT64)memMap.EntryCount);
    SerialWriteString("\r\n  Descriptor size  : ");
    SerialWriteHex64((UINT64)memMap.DescriptorSize);
    SerialWriteString(" bytes\r\n  Descriptor ver.  : ");
    SerialWriteHex64((UINT64)memMap.DescriptorVersion);
    SerialWriteString("\r\n");

    /* Walk the map and sum EfiConventionalMemory (immediately usable)
     * pages, as a concrete proof the captured map is real and
     * correctly structured. Iteration uses memMap.DescriptorSize as
     * the per-entry stride, NOT sizeof(EFI_MEMORY_DESCRIPTOR) — the
     * spec explicitly allows firmware to report a larger descriptor
     * size than our struct definition (to reserve room for future
     * spec fields), and assuming they are equal is a well-known UEFI
     * programming bug this code deliberately avoids. */
    UINT64 usablePages = 0;
    UINT8 *cursor = (UINT8 *)memMap.Map;
    for (UINTN i = 0; i < memMap.EntryCount; i++) {
        EFI_MEMORY_DESCRIPTOR *desc = (EFI_MEMORY_DESCRIPTOR *)cursor;
        if (desc->Type == EfiConventionalMemory) {
            usablePages += desc->NumberOfPages;
        }
        cursor += memMap.DescriptorSize;
    }

    /* Each page is 4 KiB per the UEFI/x86_64 architecture definition. */
    UINT64 usableMiB = (usablePages * 4096) / (1024 * 1024);

    SerialWriteString("  Usable (Conventional) pages : ");
    SerialWriteHex64(usablePages);
    SerialWriteString("\r\n  Usable (Conventional) MiB   : ");
    SerialWriteHex64(usableMiB);
    SerialWriteString("\r\n\r\n");

    SerialWriteString("Part 3 complete: memory map retrieved and Boot Services exited cleanly.\r\n");
    SerialWriteString("Halting (Part 4 will load the kernel and hand off this memory map).\r\n");

    for (;;) {
        __asm__ __volatile__("hlt");
    }

halt:
    ConOut->OutputString(ConOut, L"\r\nHalting.\r\n");
    for (;;) {
        __asm__ __volatile__("hlt");
    }

    return EFI_SUCCESS;
}
