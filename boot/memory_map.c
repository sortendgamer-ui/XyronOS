/*
 * memory_map.c — see memory_map.h for the contract and rationale.
 */

#include "include/memory_map.h"

/* Bounded retry count for the ExitBootServices sequence. The spec does
 * not define a maximum, but an unbounded loop risks hanging the
 * machine forever if something is pathologically wrong (e.g. a
 * firmware bug that invalidates the key on every single attempt). In
 * correctly behaving firmware, one retry is normally sufficient — the
 * map only changes due to our own AllocatePool call for the map buffer
 * itself, which happens once per attempt. Five attempts gives ample
 * margin over that without risking an infinite hang. */
#define MAX_EXIT_ATTEMPTS 5

/* Extra slack (in descriptor-sized units) added to the buffer size
 * GetMemoryMap's size-query call reports. Between that size query and
 * our subsequent AllocatePool call for the buffer, AllocatePool's own
 * bookkeeping can itself grow the memory map by a small number of
 * entries (e.g. splitting a free region). Without slack, the very
 * next GetMemoryMap call could return EFI_BUFFER_TOO_SMALL again for a
 * buffer we just sized to fit — this is a well-documented UEFI
 * programming pitfall, and the slack avoids it rather than looping
 * indefinitely on buffer growth. */
#define MEMORY_MAP_SLACK_ENTRIES 8

BOOLEAN ExitBootServicesWithRetry(
    EFI_SYSTEM_TABLE *SystemTable,
    EFI_HANDLE        ImageHandle,
    BOOT_MEMORY_MAP  *OutMemoryMap
)
{
    EFI_BOOT_SERVICES *BS = SystemTable->BootServices;

    for (int attempt = 0; attempt < MAX_EXIT_ATTEMPTS; attempt++) {
        /* Step 1: ask GetMemoryMap how large a buffer it needs. Per
         * spec, calling with MemoryMapSize too small (0, here) returns
         * EFI_BUFFER_TOO_SMALL and still writes the required size into
         * MemoryMapSize — we deliberately ignore the returned status
         * here and only use the size, since EFI_BUFFER_TOO_SMALL is
         * the expected, not exceptional, outcome of this call. */
        UINTN mapSize = 0;
        UINTN mapKey = 0;
        UINTN descriptorSize = 0;
        UINT32 descriptorVersion = 0;

        BS->GetMemoryMap(&mapSize, 0, &mapKey, &descriptorSize, &descriptorVersion);

        if (descriptorSize == 0) {
            /* Firmware did not report a descriptor size — something is
             * badly wrong; we cannot safely proceed. */
            return FALSE;
        }

        mapSize += descriptorSize * MEMORY_MAP_SLACK_ENTRIES;

        /* Step 2: allocate the buffer. THIS is the call most likely to
         * invalidate whatever map key we eventually get, because it is
         * itself a memory allocation — which is exactly why we get the
         * key AFTER this allocation, not before. */
        EFI_MEMORY_DESCRIPTOR *mapBuffer = 0;
        EFI_STATUS status = BS->AllocatePool(EfiLoaderData, mapSize, (VOID **)&mapBuffer);
        if (EFI_ERROR(status)) {
            return FALSE; /* Cannot proceed without a map buffer. */
        }

        /* Step 3: the real GetMemoryMap call, immediately after the
         * allocation above, with no other Boot Services calls in
         * between. The MapKey this returns corresponds to the memory
         * state at exactly this instant. */
        status = BS->GetMemoryMap(&mapSize, mapBuffer, &mapKey, &descriptorSize, &descriptorVersion);
        if (EFI_ERROR(status)) {
            BS->FreePool(mapBuffer);
            continue; /* Retry from scratch — buffer size may have
                          changed again; re-query rather than assume. */
        }

        /* Step 4: attempt the exit. THIS is the only call in the loop
         * that, on success, means Boot Services no longer exist —
         * everything before this point in this iteration was still
         * running with full Boot Services available. */
        status = BS->ExitBootServices(ImageHandle, mapKey);
        if (status == EFI_SUCCESS) {
            OutMemoryMap->Map = mapBuffer;
            OutMemoryMap->MapSizeBytes = mapSize;
            OutMemoryMap->DescriptorSize = descriptorSize;
            OutMemoryMap->DescriptorVersion = descriptorVersion;
            OutMemoryMap->EntryCount = mapSize / descriptorSize;
            /* mapBuffer is deliberately NOT freed here — FreePool is a
             * Boot Service, and Boot Services no longer exist as of
             * the line above. This allocation is now permanently ours
             * to keep (and, starting Part 4, to pass to the kernel). */
            return TRUE;
        }

        /* ExitBootServices failed — per spec, this means the map key
         * was stale (something changed the memory map after our
         * GetMemoryMap call above, before this ExitBootServices call
         * reached firmware). Free this attempt's buffer (Boot
         * Services are still active, so FreePool is still valid) and
         * loop around to try again with a freshly retrieved map. */
        BS->FreePool(mapBuffer);
    }

    /* Retry budget exhausted without a successful exit. Boot Services
     * are still active — the caller can still report this failure
     * through ConOut. */
    return FALSE;
}
