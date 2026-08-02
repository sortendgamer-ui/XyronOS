/*
 * kernel_loader.c — see kernel_loader.h for the contract.
 */

#include "include/kernel_loader.h"
#include "include/elf.h"
#include "include/boot_defs.h"

/*
 * ValidateElfHeader — check every field ADR-005 requires before this
 * bootloader will trust the file as a loadable kernel. Each check is
 * a real structural/format validation, not a formality — a
 * bootloader that skips these and just starts copying segment data
 * from an arbitrary file is exactly the "shortcut" the project rules
 * prohibit.
 */
static BOOLEAN ValidateElfHeader(Elf64_Ehdr *hdr)
{
    if (hdr->e_ident[EI_MAG0] != ELFMAG0 || hdr->e_ident[EI_MAG1] != ELFMAG1 ||
        hdr->e_ident[EI_MAG2] != ELFMAG2 || hdr->e_ident[EI_MAG3] != ELFMAG3) {
        return FALSE; /* not an ELF file at all */
    }
    if (hdr->e_ident[EI_CLASS] != ELFCLASS64) {
        return FALSE; /* not 64-bit */
    }
    if (hdr->e_ident[EI_DATA] != ELFDATA2LSB) {
        return FALSE; /* not little-endian, which x86_64 always is */
    }
    if (hdr->e_machine != EM_X86_64) {
        return FALSE; /* wrong target architecture */
    }
    if (hdr->e_type != ET_EXEC) {
        return FALSE; /* PIE/ET_DYN and relocatable ET_REL images are
                          out of scope per ADR-005 */
    }
    if (hdr->e_phnum == 0 || hdr->e_phentsize == 0) {
        return FALSE; /* no program headers means nothing to load */
    }
    return TRUE;
}

BOOLEAN LoadKernelElf(
    EFI_BOOT_SERVICES  *BS,
    EFI_FILE_PROTOCOL  *Root,
    CHAR16             *Path,
    LOADED_KERNEL      *OutKernel
)
{
    EFI_STATUS status;

    /* ---- Step 1: open and read the whole file into a temporary
     * pool buffer. This buffer is bootloader-internal scratch space —
     * unlike the kernel's final destination, nothing about it needs
     * to be reachable by the kernel after the jump, so an ordinary
     * AllocatePool (no address constraint) is correct here, same as
     * Part 2's file-read pattern. */
    EFI_FILE_PROTOCOL *file = 0;
    status = Root->Open(Root, &file, Path, EFI_FILE_MODE_READ, 0);
    if (EFI_ERROR(status)) {
        return FALSE;
    }

    EFI_GUID fileInfoGuid = EFI_FILE_INFO_ID;
    UINTN infoSize = 0;
    file->GetInfo(file, &fileInfoGuid, &infoSize, 0);

    EFI_FILE_INFO *fileInfo = 0;
    status = BS->AllocatePool(EfiLoaderData, infoSize, (VOID **)&fileInfo);
    if (EFI_ERROR(status)) {
        file->Close(file);
        return FALSE;
    }
    status = file->GetInfo(file, &fileInfoGuid, &infoSize, fileInfo);
    if (EFI_ERROR(status)) {
        BS->FreePool(fileInfo);
        file->Close(file);
        return FALSE;
    }

    UINTN fileSize = (UINTN)fileInfo->FileSize;
    BS->FreePool(fileInfo);

    UINT8 *fileBuffer = 0;
    status = BS->AllocatePool(EfiLoaderData, fileSize, (VOID **)&fileBuffer);
    if (EFI_ERROR(status)) {
        file->Close(file);
        return FALSE;
    }

    status = file->Read(file, &fileSize, fileBuffer);
    file->Close(file);
    if (EFI_ERROR(status)) {
        BS->FreePool(fileBuffer);
        return FALSE;
    }

    /* ---- Step 2: parse and validate the ELF header. ---- */
    if (fileSize < sizeof(Elf64_Ehdr)) {
        BS->FreePool(fileBuffer);
        return FALSE;
    }
    Elf64_Ehdr *ehdr = (Elf64_Ehdr *)fileBuffer;
    if (!ValidateElfHeader(ehdr)) {
        BS->FreePool(fileBuffer);
        return FALSE;
    }

    /* ---- Step 3: walk PT_LOAD program headers to compute the total
     * virtual address span the kernel needs, using e_phentsize as the
     * per-entry stride rather than assuming sizeof(Elf64_Phdr) — the
     * same non-fixed-stride lesson applied to the UEFI memory map in
     * Part 3 applies here too, since the ELF spec permits a header
     * larger than what a given loader was compiled to know about. */
    UINT64 minVAddr = 0xFFFFFFFFFFFFFFFFULL;
    UINT64 maxVAddrEnd = 0;
    BOOLEAN foundAnyLoadSegment = FALSE;

    for (UINT16 i = 0; i < ehdr->e_phnum; i++) {
        UINT8 *phdrBytes = fileBuffer + ehdr->e_phoff + (UINTN)i * ehdr->e_phentsize;
        Elf64_Phdr *phdr = (Elf64_Phdr *)phdrBytes;

        if (phdr->p_type != PT_LOAD) {
            continue;
        }
        if (phdr->p_vaddr < KERNEL_VIRTUAL_BASE) {
            /* Per ADR-005, every loadable segment must live in the
             * higher-half kernel region — a segment below that would
             * fall outside the mapping paging.c is about to build. */
            BS->FreePool(fileBuffer);
            return FALSE;
        }

        foundAnyLoadSegment = TRUE;
        if (phdr->p_vaddr < minVAddr) {
            minVAddr = phdr->p_vaddr;
        }
        UINT64 segmentEnd = phdr->p_vaddr + phdr->p_memsz;
        if (segmentEnd > maxVAddrEnd) {
            maxVAddrEnd = segmentEnd;
        }
    }

    if (!foundAnyLoadSegment) {
        BS->FreePool(fileBuffer);
        return FALSE;
    }

    UINT64 kernelSpan = maxVAddrEnd - minVAddr;
    UINT64 alignedSize = RoundUpTo(kernelSpan, PAGE_SIZE_2M);
    if (alignedSize == 0 || alignedSize > KERNEL_MAX_SIZE_BYTES) {
        BS->FreePool(fileBuffer);
        return FALSE;
    }
    if (ehdr->e_entry < minVAddr || ehdr->e_entry >= maxVAddrEnd) {
        /* Entry point falls outside every loaded segment — the file
         * is malformed or was not built the way this bootloader
         * expects. */
        BS->FreePool(fileBuffer);
        return FALSE;
    }

    /* ---- Step 4: allocate the kernel's final physical home. Capped
     * below IDENTITY_MAP_LIMIT (ADR-005) so it stays reachable, and
     * over-allocated by one extra 2 MiB block so we can hand back a
     * 2 MiB-aligned base — AllocatePages only guarantees 4 KiB
     * alignment on its own. */
    UINT64 pagesNeeded4K = (alignedSize + PAGE_SIZE_2M) / PAGE_SIZE_4K;
    EFI_PHYSICAL_ADDRESS physAllocation = IDENTITY_MAP_LIMIT - 1;
    status = BS->AllocatePages(AllocateMaxAddress, EfiLoaderData, pagesNeeded4K, &physAllocation);
    if (EFI_ERROR(status)) {
        BS->FreePool(fileBuffer);
        return FALSE;
    }

    UINT64 physBase = RoundUpTo((UINT64)physAllocation, PAGE_SIZE_2M);

    /* ---- Step 5: zero the whole destination first (correctly
     * handles BSS — the memsz-minus-filesz tail of any segment, and
     * any padding between segments, must read as zero, not whatever
     * was previously in that physical memory), then copy each
     * segment's file bytes to its correct offset. */
    UINT8 *destBase = (UINT8 *)(UINTN)physBase;
    for (UINT64 i = 0; i < alignedSize; i++) {
        destBase[i] = 0;
    }

    for (UINT16 i = 0; i < ehdr->e_phnum; i++) {
        UINT8 *phdrBytes = fileBuffer + ehdr->e_phoff + (UINTN)i * ehdr->e_phentsize;
        Elf64_Phdr *phdr = (Elf64_Phdr *)phdrBytes;
        if (phdr->p_type != PT_LOAD) {
            continue;
        }

        UINT64 destOffset = phdr->p_vaddr - minVAddr;
        UINT8 *dest = destBase + destOffset;
        UINT8 *src = fileBuffer + phdr->p_offset;

        for (UINT64 b = 0; b < phdr->p_filesz; b++) {
            dest[b] = src[b];
        }
        /* Bytes [p_filesz, p_memsz) were already zeroed above and are
         * intentionally not touched here — that is BSS. */
    }

    /* Capture every field still needed from ehdr/fileBuffer into local
     * variables BEFORE freeing fileBuffer — ehdr is a pointer INTO
     * fileBuffer, so reading through it after FreePool is a
     * use-after-free. (Caught by actually booting this in QEMU: the
     * entry point printed as 0xAFAFAFAFAFAFAFAF, EDK2's freed-pool
     * debug scrub pattern, which is exactly what reading freed memory
     * looks like — a concrete demonstration of why every part of this
     * project is boot-tested, not just compiled.) */
    UINT64 entryPoint = ehdr->e_entry;

    BS->FreePool(fileBuffer);

    OutKernel->KernelPhysicalBase = physBase;
    OutKernel->KernelVirtualBase  = minVAddr;
    OutKernel->KernelSizeBytes    = alignedSize;
    OutKernel->EntryPointVirtual  = entryPoint;

    return TRUE;
}
