/*
 * paging.h — Build the page tables the kernel starts running under.
 * See ADR-005 for the full design rationale.
 */

#ifndef OS_PAGING_H
#define OS_PAGING_H

#include "efi_types.h"
#include "efi_boot_services.h"

/*
 * BuildKernelPageTables — allocate and populate a fresh PML4 mapping:
 *   - Identity map of physical [0, IDENTITY_MAP_LIMIT) using 2 MiB pages.
 *   - Higher-half mapping of KERNEL_VIRTUAL_BASE to
 *     [KernelPhysicalBase, KernelPhysicalBase + KernelSizeBytes),
 *     also using 2 MiB pages, per ADR-002/ADR-005.
 *
 * KernelSizeBytes must already be 2 MiB-aligned (kernel_loader.c
 * guarantees this when it reserves the kernel's physical pages).
 *
 * All page table pages themselves are allocated via BS->AllocatePages
 * with EfiLoaderData — Boot Services are still active at this point
 * in main.c's flow (this runs BEFORE ExitBootServicesWithRetry), so
 * ordinary boot-time allocation is fine here.
 *
 * Returns the physical address to load into CR3 (the PML4 table's own
 * physical address, page-aligned, flags clear) on success, or 0 on
 * failure (allocation failure — extremely unlikely this early in boot
 * with boot services still fully available, but checked rather than
 * assumed, per the project's no-shortcuts requirement).
 */
UINT64 BuildKernelPageTables(
    EFI_BOOT_SERVICES *BS,
    UINT64             KernelPhysicalBase,
    UINT64             KernelSizeBytes
);

#endif /* OS_PAGING_H */
