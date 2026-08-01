/*
 * efi_loaded_image_protocol.h — EFI_LOADED_IMAGE_PROTOCOL.
 *
 * When firmware loads our BOOTX64.EFI, it installs this protocol on our
 * own ImageHandle. The field we actually need is DeviceHandle: the
 * handle of the volume we were loaded from. We open the Simple File
 * System Protocol on that same handle (main.c) so file reads happen
 * against the correct disk even on a machine with multiple boot devices
 * — hardcoding "the first FAT volume found" would silently break on
 * such a machine.
 *
 * Reference: UEFI Specification 2.10, Section 9.1
 * (EFI_LOADED_IMAGE_PROTOCOL).
 */

#ifndef OS_EFI_LOADED_IMAGE_PROTOCOL_H
#define OS_EFI_LOADED_IMAGE_PROTOCOL_H

#include "efi_types.h"
#include "efi_tables.h"
#include "efi_boot_services.h"

#define EFI_LOADED_IMAGE_PROTOCOL_GUID \
    { 0x5B1B31A1, 0x9562, 0x11d2, \
      { 0x8E, 0x3F, 0x00, 0xA0, 0xC9, 0x69, 0x72, 0x3B } }

#define EFI_LOADED_IMAGE_PROTOCOL_REVISION 0x1000

typedef struct {
    UINT32 Revision;

    EFI_HANDLE ParentHandle;
    EFI_SYSTEM_TABLE *SystemTable;

    /* Source location of the image. DeviceHandle is the field this
     * bootloader reads. */
    EFI_HANDLE                DeviceHandle;
    EFI_DEVICE_PATH_PROTOCOL  *FilePath;
    VOID                      *Reserved;

    /* Image's load options — unused by this bootloader (no command
     * line arguments are passed to it by the firmware boot manager
     * in our use case), but present here for correct struct layout. */
    UINT32 LoadOptionsSize;
    VOID   *LoadOptions;

    /* Location where the image was loaded. */
    VOID   *ImageBase;
    UINT64 ImageSize;
    EFI_MEMORY_TYPE ImageCodeType;
    EFI_MEMORY_TYPE ImageDataType;

    /* Function pointer type intentionally left opaque (VOID*) — this is
     * EFI_IMAGE_UNLOAD, already fully defined in efi_boot_services.h;
     * duplicating that typedef here would risk the two definitions
     * drifting apart. Cast at the (rare) call site instead. */
    VOID *Unload;
} EFI_LOADED_IMAGE_PROTOCOL;

#endif /* OS_EFI_LOADED_IMAGE_PROTOCOL_H */
