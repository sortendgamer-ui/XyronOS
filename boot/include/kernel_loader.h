/*
 * kernel_loader.h — Load an ET_EXEC ELF64 kernel image off the ESP
 * into a bootloader-chosen physical location, ready for paging.c to
 * map and trampoline.asm to jump into. See ADR-005.
 */

#ifndef OS_KERNEL_LOADER_H
#define OS_KERNEL_LOADER_H

#include "efi_types.h"
#include "efi_tables.h"
#include "efi_boot_services.h"
#include "efi_file_protocol.h"

typedef struct {
    UINT64 KernelPhysicalBase;  /* 2 MiB-aligned physical load address */
    UINT64 KernelVirtualBase;   /* == the ELF's lowest PT_LOAD p_vaddr,
                                    expected to equal KERNEL_VIRTUAL_BASE */
    UINT64 KernelSizeBytes;     /* 2 MiB-aligned; the span paging.c must map */
    UINT64 EntryPointVirtual;   /* ELF e_entry — jump target, once mapped */
} LOADED_KERNEL;

/*
 * LoadKernelElf — open, validate, and load an ET_EXEC ELF64 image
 * from the given root directory.
 *
 * Validates: ELF magic, ELFCLASS64, ELFDATA2LSB, EM_X86_64, ET_EXEC,
 * that every PT_LOAD segment's virtual address is at or above
 * KERNEL_VIRTUAL_BASE, and that the total image size fits within
 * KERNEL_MAX_SIZE_BYTES (boot_defs.h) — all real validation, not
 * merely trusting the file to be well-formed.
 *
 * Returns TRUE and populates OutKernel on success. Returns FALSE on
 * any I/O error or validation failure — the caller (main.c) treats
 * this as fatal and halts via the existing ConOut error-reporting
 * path, since Boot Services are still active whenever this function
 * runs (it is always called before ExitBootServicesWithRetry).
 */
BOOLEAN LoadKernelElf(
    EFI_BOOT_SERVICES  *BS,
    EFI_FILE_PROTOCOL  *Root,
    CHAR16             *Path,
    LOADED_KERNEL      *OutKernel
);

#endif /* OS_KERNEL_LOADER_H */
