/*
 * boot_info.h — The BootInfo handoff structure.
 *
 * This is the ONLY thing the kernel knows about the bootloader that
 * ran before it: a pointer to one of these, in RDI, at kernel entry
 * (see ADR-005 for the calling-convention reasoning and
 * trampoline.asm for where RDI actually gets set).
 *
 * Because the bootloader and the eventual kernel are built by
 * completely separate toolchains (mingw-w64 PE vs. the kernel's own
 * ELF/Rust toolchain from Phase 3 onward) that never share a build
 * step, this struct's binary layout IS the interface — there is no
 * compiler to catch a mismatch. Magic + Version exist so a kernel can
 * detect a mismatched or corrupt handoff and refuse to trust the rest
 * of the struct, rather than silently reading garbage.
 */

#ifndef OS_BOOT_INFO_H
#define OS_BOOT_INFO_H

#include "efi_types.h"

/* ASCII "XOSBOOT1" read as a little-endian UINT64. Written as a
 * byte-by-byte construction rather than a single hex literal so the
 * magic's meaning is legible directly in source, not just in a
 * comment next to an opaque constant. */
#define BOOTINFO_MAGIC \
    ((UINT64)'X' | ((UINT64)'O' << 8) | ((UINT64)'S' << 16) | \
     ((UINT64)'B' << 24) | ((UINT64)'O' << 32) | ((UINT64)'O' << 40) | \
     ((UINT64)'T' << 48) | ((UINT64)'1' << 56))

/* Bump this whenever a field is added, removed, reordered, or
 * reinterpreted. A kernel built against version N must refuse to
 * proceed if handed a struct reporting any version other than N —
 * see ADR-005. */
#define BOOTINFO_VERSION 1

typedef struct {
    UINT64 Magic;             /* Must equal BOOTINFO_MAGIC. */
    UINT32 Version;           /* Must equal BOOTINFO_VERSION for the
                                  kernel reading this struct. */
    UINT32 StructSizeBytes;   /* sizeof(BOOT_INFO) as the bootloader
                                  compiled it — lets a kernel detect a
                                  size mismatch even if Magic/Version
                                  somehow both matched by coincidence. */

    /* Final UEFI memory map, captured in memory_map.c immediately
     * before ExitBootServices succeeded. MemoryMapPhysAddr is a
     * physical address, valid to dereference directly because it
     * falls within the identity-mapped region paging.c builds (see
     * ADR-005) — the same page tables are still active when the
     * kernel starts, since nothing changes CR3 again between the
     * bootloader's switch and the kernel taking over. */
    UINT64 MemoryMapPhysAddr;
    UINT64 MemoryMapSizeBytes;
    UINT64 MemoryMapDescriptorSize;
    UINT32 MemoryMapDescriptorVersion;
    UINT64 MemoryMapEntryCount;

    /* Where the bootloader placed the kernel image, and how big the
     * region it reserved is (rounded up to a 2 MiB boundary — see
     * paging.c). KernelVirtualBase is always KERNEL_VIRTUAL_BASE
     * (boot_defs.h) today, included explicitly anyway so the kernel
     * never needs to hardcode a value the bootloader already knows
     * authoritatively. */
    UINT64 KernelPhysicalBase;
    UINT64 KernelVirtualBase;
    UINT64 KernelSizeBytes;

    /* A dedicated stack the bootloader allocates and switches to
     * (trampoline.asm) immediately before the jump, rather than
     * leaving the kernel running on whatever stack UEFI's own
     * pre-ExitBootServices environment happened to be using —
     * inheriting an unverified, unowned stack region would be exactly
     * the kind of shortcut ADR-005 and this project's rules rule out.
     * StackTop is the initial RSP value (stack grows down from here);
     * StackSizeBytes is its total size, so the kernel can know its
     * own bounds rather than assume them. */
    UINT64 KernelStackTop;
    UINT64 KernelStackSizeBytes;
} BOOT_INFO;

#endif /* OS_BOOT_INFO_H */
