/*
 * paging.c — see paging.h and ADR-005 for the design.
 *
 * Page table entry format here (Present/Writable/PS bits, physical
 * address in bits 51:12) is dictated by the x86_64 architecture's own
 * documented paging structures (Intel SDM Vol. 3A, AMD64 Architecture
 * Programmer's Manual Vol. 2) — an external hardware specification,
 * implemented independently, not code taken from an existing OS.
 */

#include "include/paging.h"
#include "include/boot_defs.h"

#define PTE_PRESENT  0x1ULL
#define PTE_WRITABLE 0x2ULL
#define PTE_PS       0x80ULL /* "Page Size": at the PDPT or PD level,
                                 set means this entry is a huge page
                                 (1 GiB or 2 MiB respectively) rather
                                 than a pointer to the next table. */

#define ENTRIES_PER_TABLE 512
#define ONE_GIB 0x40000000ULL

/*
 * AllocateZeroedPage — allocate one 4 KiB page for use as a page
 * table, and zero it. EFI_ALLOCATE_PAGES does not guarantee
 * zero-initialized memory, and a page table with garbage in
 * not-yet-populated entries would have its Present bit (bit 0) set
 * "by accident" on essentially random data, which the CPU's page
 * walker would then follow into undefined behavior — zeroing is not
 * optional here.
 */
static UINT64 *AllocateZeroedPage(EFI_BOOT_SERVICES *BS)
{
    EFI_PHYSICAL_ADDRESS addr = 0;
    EFI_STATUS status = BS->AllocatePages(AllocateAnyPages, EfiLoaderData, 1, &addr);
    if (EFI_ERROR(status)) {
        return 0;
    }

    UINT64 *page = (UINT64 *)(UINTN)addr;
    for (UINTN i = 0; i < (PAGE_SIZE_4K / sizeof(UINT64)); i++) {
        page[i] = 0;
    }
    return page;
}

UINT64 BuildKernelPageTables(
    EFI_BOOT_SERVICES *BS,
    UINT64             KernelPhysicalBase,
    UINT64             KernelSizeBytes
)
{
    UINT64 *pml4 = AllocateZeroedPage(BS);
    if (pml4 == 0) {
        return 0;
    }

    /* ==== Region 1: identity map of [0, IDENTITY_MAP_LIMIT) ==========
     * One PML4 entry (index 0, covering the low 512 GiB of virtual
     * address space) -> one PDPT -> one PD per GiB, each PD fully
     * populated with 2 MiB pages. See ADR-005 for why this range and
     * this granularity. */
    UINT64 *identityPdpt = AllocateZeroedPage(BS);
    if (identityPdpt == 0) {
        return 0;
    }

    UINT64 identityGibCount = IDENTITY_MAP_LIMIT / ONE_GIB;
    for (UINT64 gib = 0; gib < identityGibCount; gib++) {
        UINT64 *pd = AllocateZeroedPage(BS);
        if (pd == 0) {
            return 0;
        }
        for (UINT64 entry = 0; entry < ENTRIES_PER_TABLE; entry++) {
            UINT64 physAddr = gib * ONE_GIB + entry * PAGE_SIZE_2M;
            pd[entry] = physAddr | PTE_PRESENT | PTE_WRITABLE | PTE_PS;
        }
        identityPdpt[gib] = ((UINT64)(UINTN)pd) | PTE_PRESENT | PTE_WRITABLE;
    }
    pml4[0] = ((UINT64)(UINTN)identityPdpt) | PTE_PRESENT | PTE_WRITABLE;

    /* ==== Region 2: higher-half kernel mapping =======================
     * KERNEL_VIRTUAL_BASE (0xFFFFFFFF80000000) decodes to PML4 index
     * 511, PDPT index 510 (see ADR-002/ADR-005 derivation) -> one PD,
     * populated with exactly as many 2 MiB entries as the kernel
     * image needs, starting at PD index 0. */
    UINT64 *kernelPdpt = AllocateZeroedPage(BS);
    if (kernelPdpt == 0) {
        return 0;
    }
    UINT64 *kernelPd = AllocateZeroedPage(BS);
    if (kernelPd == 0) {
        return 0;
    }

    UINT64 kernelPageCount = KernelSizeBytes / PAGE_SIZE_2M;
    if (kernelPageCount == 0) {
        kernelPageCount = 1; /* A non-empty image always needs at
                                 least one 2 MiB page mapped. */
    }
    if (kernelPageCount > ENTRIES_PER_TABLE) {
        /* Exceeds the single-PD-table capacity this bootloader
         * supports (512 * 2 MiB = 1 GiB) — see ADR-005 "Consequences"
         * for why this limit exists and is not a practical concern
         * today. Fail loudly rather than silently truncate the
         * mapping, which would leave part of the kernel unmapped and
         * cause a page fault the moment it was touched. */
        return 0;
    }

    for (UINT64 entry = 0; entry < kernelPageCount; entry++) {
        UINT64 physAddr = KernelPhysicalBase + entry * PAGE_SIZE_2M;
        kernelPd[entry] = physAddr | PTE_PRESENT | PTE_WRITABLE | PTE_PS;
    }

    kernelPdpt[510] = ((UINT64)(UINTN)kernelPd) | PTE_PRESENT | PTE_WRITABLE;
    pml4[511] = ((UINT64)(UINTN)kernelPdpt) | PTE_PRESENT | PTE_WRITABLE;

    /* CR3's low 12 bits are flags (PCD/PWT) we leave at 0 for default
     * write-back caching — the PML4 table itself is already
     * 4 KiB-aligned (AllocatePages always returns page-aligned
     * memory), so no masking is needed before returning it as-is. */
    return (UINT64)(UINTN)pml4;
}
