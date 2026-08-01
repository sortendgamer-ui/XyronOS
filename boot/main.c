/*
 * main.c — Bootloader entry point, Phase 2 Part 2.
 *
 * Part 1 proved the toolchain, struct layout, and entry point were all
 * correct by printing to the console and halting. Part 2 proves the
 * file-I/O path works: from our own image handle, find the volume we
 * were loaded from, open its root directory, open a test file, read
 * its contents into a heap buffer, and print them. Every step here is
 * exactly what Part 4 will do again to load the real kernel image —
 * this part exists so that when Part 4 does it for the kernel, the
 * file-I/O plumbing itself is already verified working, and a Part 4
 * failure can only mean a kernel-loading-specific bug, not a protocol
 * bug.
 *
 * What this program deliberately does NOT do yet:
 *   - Call GetMemoryMap / ExitBootServices (Part 3)
 *   - Load an actual kernel or jump to it (Part 4 — no kernel exists
 *     until Phase 3 begins)
 */

#include "include/efi_types.h"
#include "include/efi_tables.h"
#include "include/efi_boot_services.h"
#include "include/efi_loaded_image_protocol.h"
#include "include/efi_file_protocol.h"

/*
 * PrintAscii — convert and print a narrow (CHAR8/ASCII) buffer through
 * the UEFI console, which only accepts CHAR16 (UCS-2) strings.
 *
 * This exists because the test file we read is plain ASCII text on
 * disk, but EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL->OutputString requires a
 * null-terminated CHAR16 string. We convert in small fixed-size chunks
 * on the stack rather than allocating a second full-size heap buffer,
 * since the conversion buffer's lifetime is only this function call.
 */
static void PrintAscii(EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *ConOut,
                        CHAR8 *buffer, UINTN length)
{
    CHAR16 chunk[128];
    UINTN chunkLen = 0;

    for (UINTN i = 0; i < length; i++) {
        /* Firmware's console expects CR before LF for correct
         * positioning, same as the string literals elsewhere in this
         * file — translate bare '\n' from the text file accordingly. */
        if (buffer[i] == '\n' && (chunkLen == 0 || chunk[chunkLen - 1] != L'\r')) {
            chunk[chunkLen++] = L'\r';
        }

        chunk[chunkLen++] = (CHAR16)buffer[i];

        /* Flush when the chunk buffer is nearly full (leave room for a
         * null terminator plus one more possible \r\n pair) or when
         * this is the last byte of input. */
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

    ConOut->ClearScreen(ConOut);
    ConOut->OutputString(ConOut, L"XyronOS Bootloader \x2014 Phase 2, Part 2\r\n");
    ConOut->OutputString(ConOut, L"Testing Simple File System read pipeline...\r\n\r\n");

    /* Step 1: get our own EFI_LOADED_IMAGE_PROTOCOL so we know which
     * volume (DeviceHandle) we were loaded from. HandleProtocol is the
     * simplest correct call here — we are not a driver managing a
     * controller relationship, so the extra bookkeeping OpenProtocol
     * offers (AgentHandle/ControllerHandle/Attributes) has no benefit
     * for this one-shot lookup. */
    EFI_GUID loadedImageGuid = EFI_LOADED_IMAGE_PROTOCOL_GUID;
    EFI_LOADED_IMAGE_PROTOCOL *loadedImage = 0;

    status = BS->HandleProtocol(ImageHandle, &loadedImageGuid, (VOID **)&loadedImage);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: could not get LoadedImageProtocol\r\n");
        goto halt;
    }
    ConOut->OutputString(ConOut, L"[OK] LoadedImageProtocol acquired.\r\n");

    /* Step 2: get the Simple File System Protocol installed on that
     * same device handle — this is the firmware's FAT driver for the
     * volume we booted from. */
    EFI_GUID sfspGuid = EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID;
    EFI_SIMPLE_FILE_SYSTEM_PROTOCOL *sfsp = 0;

    status = BS->HandleProtocol(loadedImage->DeviceHandle, &sfspGuid, (VOID **)&sfsp);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: could not get SimpleFileSystemProtocol\r\n");
        goto halt;
    }
    ConOut->OutputString(ConOut, L"[OK] SimpleFileSystemProtocol acquired.\r\n");

    /* Step 3: open the volume's root directory. */
    EFI_FILE_PROTOCOL *root = 0;
    status = sfsp->OpenVolume(sfsp, &root);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: OpenVolume failed\r\n");
        goto halt;
    }
    ConOut->OutputString(ConOut, L"[OK] Root directory opened.\r\n");

    /* Step 4: open the test file. Path is relative to the volume
     * root and uses backslashes, per FAT/UEFI path convention. */
    EFI_FILE_PROTOCOL *testFile = 0;
    status = root->Open(root, &testFile, L"\\BOOTINFO.TXT",
                         EFI_FILE_MODE_READ, 0);
    if (EFI_ERROR(status)) {
        ConOut->OutputString(ConOut, L"FAILED: could not open \\BOOTINFO.TXT\r\n");
        ConOut->OutputString(ConOut, L"(Did you copy boot/testdata/BOOTINFO.TXT to the ESP root? See README.)\r\n");
        goto halt;
    }
    ConOut->OutputString(ConOut, L"[OK] \\BOOTINFO.TXT opened.\r\n");

    /* Step 5: query the file's size via GetInfo before reading, so we
     * allocate exactly the right buffer instead of guessing. GetInfo's
     * two-call pattern (call once with a too-small buffer to learn the
     * required size, then again with a correctly sized one) is the
     * standard UEFI idiom for variable-sized data — EFI_FILE_INFO's
     * FileName field can be any length, so its total size is not
     * knowable at compile time. */
    EFI_GUID fileInfoGuid = EFI_FILE_INFO_ID;
    UINTN infoSize = 0;
    testFile->GetInfo(testFile, &fileInfoGuid, &infoSize, 0);
    /* First call is expected to return EFI_BUFFER_TOO_SMALL and fill in
     * infoSize; we deliberately ignore its status and only use the
     * size it reported, since a zero infoSize after this call would
     * still be caught by the AllocatePool failure path below. */

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

    /* Step 6: allocate a buffer for the file's contents and read it in
     * one call. UINTN FileSize is documented (Spec 13.5) to be the
     * file's size in bytes for a non-directory file. */
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

    /* Step 7: print what we read, proving the bytes on disk actually
     * made it into memory correctly. */
    PrintAscii(ConOut, fileData, fileDataSize);

    ConOut->OutputString(ConOut, L"------------------------------------------------------------\r\n");
    ConOut->OutputString(ConOut, L"[OK] Part 2 file-read pipeline verified successfully.\r\n");

    /* Step 8: clean up. Even though the whole program is about to halt
     * (and Part 3's ExitBootServices will eventually make all of this
     * moot for the final kernel handoff), a bootloader that leaves
     * every resource dangling is bad practice we don't want to carry
     * forward as a habit into later, longer-lived parts. */
freeData:
    BS->FreePool(fileData);
freeInfo:
    BS->FreePool(fileInfo);
closeFile:
    testFile->Close(testFile);

halt:
    ConOut->OutputString(ConOut, L"\r\nHalting (Part 2 has no further tasks).\r\n");
    for (;;) {
        __asm__ __volatile__("hlt");
    }

    return EFI_SUCCESS;
}
