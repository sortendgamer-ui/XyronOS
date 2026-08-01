/*
 * memory_map.h — Correct GetMemoryMap + ExitBootServices sequencing.
 *
 * The UEFI spec's ExitBootServices contract (Section 7.4) requires the
 * caller to pass a "map key" obtained from the most recent GetMemoryMap
 * call. If any memory allocation or free happens between that
 * GetMemoryMap call and the ExitBootServices call — including the
 * allocation of the memory map buffer itself — the key is stale and
 * ExitBootServices returns EFI_INVALID_PARAMETER. The spec's documented
 * recovery is: call GetMemoryMap again for a fresh key, then retry
 * ExitBootServices immediately, with no other Boot Services calls in
 * between. This file implements exactly that loop — see ADR reasoning
 * in the Phase 2 Part 3 changelog entry for why a single, unretried
 * call would be a spec violation, not a simplification.
 */

#ifndef OS_MEMORY_MAP_H
#define OS_MEMORY_MAP_H

#include "efi_types.h"
#include "efi_tables.h"
#include "efi_boot_services.h"

/* Bundles everything the eventual kernel handoff (Part 4) will need
 * from the final, post-exit memory map: the map itself, and the two
 * pieces of information required to walk it correctly (entry count is
 * NOT simply TotalSize / sizeof(EFI_MEMORY_DESCRIPTOR) — see
 * DescriptorSize note in memory_map.c). */
typedef struct {
    EFI_MEMORY_DESCRIPTOR *Map;        /* Pool-allocated; valid forever
                                           after a successful exit, since
                                           FreePool can no longer be
                                           called to reclaim it. */
    UINTN                  MapSizeBytes;
    UINTN                  DescriptorSize;
    UINT32                 DescriptorVersion;
    UINTN                  EntryCount;
} BOOT_MEMORY_MAP;

/*
 * ExitBootServicesWithRetry — retrieve the memory map and call
 * ExitBootServices, retrying on a stale map key as the spec requires.
 *
 * On success: returns TRUE, boot services are terminated, and
 * OutMemoryMap is populated with the final map that was current at
 * the moment of the successful exit. From this point on, the caller
 * must not call ANY EFI_BOOT_SERVICES function — this function's own
 * internal retry loop is the last code in the bootloader permitted to
 * do so.
 *
 * On failure (retry budget exhausted): returns FALSE. Boot services
 * are still active in this case — the caller may still use ConOut and
 * other boot services to report the failure, since we never reached a
 * successful ExitBootServices call.
 */
BOOLEAN ExitBootServicesWithRetry(
    EFI_SYSTEM_TABLE *SystemTable,
    EFI_HANDLE        ImageHandle,
    BOOT_MEMORY_MAP  *OutMemoryMap
);

#endif /* OS_MEMORY_MAP_H */
