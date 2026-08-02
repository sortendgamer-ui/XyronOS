/*
 * boot_defs.h — Shared constants for Part 4's kernel handoff.
 *
 * Centralized here rather than repeated as magic numbers across
 * paging.c, kernel_loader.c, and memory_map.c, per ADR-005.
 */

#ifndef OS_BOOT_DEFS_H
#define OS_BOOT_DEFS_H

#include "efi_types.h"

#define PAGE_SIZE_4K   0x1000ULL
#define PAGE_SIZE_2M   0x200000ULL

/* Every boot-time allocation the kernel will dereference directly
 * (its own image, the final memory map buffer, the BootInfo struct)
 * is capped below this physical address, matching the identity map's
 * coverage built in paging.c. See ADR-005 for why. */
#define IDENTITY_MAP_LIMIT 0x100000000ULL /* 4 GiB */

/* Kernel higher-half virtual base, per ADR-002. Repeated here (not
 * only in the ADR text) because paging.c and kernel_loader.c both
 * need it as a compile-time constant. */
#define KERNEL_VIRTUAL_BASE 0xFFFFFFFF80000000ULL

/* Maximum kernel image size this bootloader's paging code supports —
 * see ADR-005 "Consequences" for why this specific limit exists and
 * why it is not a practical concern. */
#define KERNEL_MAX_SIZE_BYTES (512ULL * PAGE_SIZE_2M) /* 1 GiB */

static inline UINT64 RoundUpTo(UINT64 value, UINT64 alignment)
{
    return (value + (alignment - 1)) & ~(alignment - 1);
}

#endif /* OS_BOOT_DEFS_H */
