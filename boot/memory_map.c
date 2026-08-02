/*
 * memory_map.c — see memory_map.h for the contract.
 *
 * Part 4 change from Part 3 (see ADR-005 "Consequences"): the final
 * memory map buffer now comes from AllocatePages(AllocateMaxAddress)
 * instead of AllocatePool, capped below IDENTITY_MAP_LIMIT, so the
 * kernel can dereference MemoryMapPhysAddr (via BootInfo) after the
 * jump — a plain AllocatePool call gives no control over where the
 * memory lands and could return an address our post-jump identity map
 * doesn't cover. The stale-map-key retry loop itself, the actual
 * subject of Part 3, is unchanged below.
 */

#include "include/memory_map.h"
#include "include/boot_defs.h"

#define MAX_EXIT_ATTEMPTS 5
#define MEMORY_MAP_SLACK_ENTRIES 8

BOOLEAN ExitBootServicesWithRetry(
    EFI_SYSTEM_TABLE *SystemTable,
    EFI_HANDLE        ImageHandle,
    BOOT_MEMORY_MAP  *OutMemoryMap
)
{
    EFI_BOOT_SERVICES *BS = SystemTable->BootServices;

    for (int attempt = 0; attempt < MAX_EXIT_ATTEMPTS; attempt++) {
        UINTN mapSize = 0;
        UINTN mapKey = 0;
        UINTN descriptorSize = 0;
        UINT32 descriptorVersion = 0;

        /* Step 1: learn the required buffer size. Ignoring the
         * returned status is intentional — EFI_BUFFER_TOO_SMALL is
         * the expected outcome of a zero-size query, not an error. */
        BS->GetMemoryMap(&mapSize, 0, &mapKey, &descriptorSize, &descriptorVersion);

        if (descriptorSize == 0) {
            return FALSE;
        }

        mapSize += descriptorSize * MEMORY_MAP_SLACK_ENTRIES;

        /* Step 2: allocate the buffer via AllocatePages, capped below
         * IDENTITY_MAP_LIMIT so the kernel can read it post-jump (see
         * file header comment and ADR-005). This allocation is itself
         * what most often invalidates the map key we're about to
         * fetch — which is exactly why the key is fetched AFTER it,
         * not before. */
        UINT64 pageCount = RoundUpTo(mapSize, PAGE_SIZE_4K) / PAGE_SIZE_4K;
        EFI_PHYSICAL_ADDRESS mapPhysAddr = IDENTITY_MAP_LIMIT - 1;
        EFI_STATUS status = BS->AllocatePages(AllocateMaxAddress, EfiLoaderData,
                                               pageCount, &mapPhysAddr);
        if (EFI_ERROR(status)) {
            return FALSE; /* Cannot proceed without a map buffer. */
        }

        EFI_MEMORY_DESCRIPTOR *mapBuffer = (EFI_MEMORY_DESCRIPTOR *)(UINTN)mapPhysAddr;

        /* Step 3: the real GetMemoryMap call, immediately after the
         * allocation above, with no other Boot Services calls in
         * between — the MapKey this returns corresponds to memory
         * state at exactly this instant. */
        status = BS->GetMemoryMap(&mapSize, mapBuffer, &mapKey, &descriptorSize, &descriptorVersion);
        if (EFI_ERROR(status)) {
            BS->FreePages(mapPhysAddr, pageCount);
            continue; /* Retry from scratch. */
        }

        /* Step 4: attempt the exit. */
        status = BS->ExitBootServices(ImageHandle, mapKey);
        if (status == EFI_SUCCESS) {
            OutMemoryMap->Map = mapBuffer;
            OutMemoryMap->MapSizeBytes = mapSize;
            OutMemoryMap->DescriptorSize = descriptorSize;
            OutMemoryMap->DescriptorVersion = descriptorVersion;
            OutMemoryMap->EntryCount = mapSize / descriptorSize;
            /* Deliberately not freed — FreePages is a Boot Service,
             * and Boot Services no longer exist as of the line above.
             * This allocation belongs to the kernel now. */
            return TRUE;
        }

        /* Stale map key — free this attempt's buffer (Boot Services
         * are still active here) and retry with a fresh map. */
        BS->FreePages(mapPhysAddr, pageCount);
    }

    return FALSE;
}
