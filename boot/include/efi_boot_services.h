/*
 * efi_boot_services.h — The full EFI_BOOT_SERVICES function table.
 *
 * Field ORDER in the struct below is dictated by UEFI Specification 2.10,
 * Section 4.4, Table 4.6. Firmware builds this table in memory in exactly
 * this order and hands us a pointer to it via EFI_SYSTEM_TABLE->BootServices
 * (efi_tables.h). Every field must be present, in this order, with a
 * pointer-sized type, even for services this bootloader does not call yet —
 * omitting or reordering any field would misalign every field after it.
 *
 * Individual function pointer signatures are filled in accurately from the
 * spec for services we call in Part 2 (AllocatePool, FreePool,
 * HandleProtocol, OpenProtocol, CloseProtocol, LocateProtocol) and for
 * services we know upcoming parts need (GetMemoryMap, AllocatePages,
 * FreePages, ExitBootServices — Part 3; LoadImage, StartImage — considered,
 * not used since we hand off by jumping directly, see ADR note in Part 4
 * when written). Every remaining field also gets its real spec signature
 * rather than a generic placeholder pointer type, because a future part
 * may need it and re-deriving these signatures piecemeal across many
 * files would risk inconsistency — this matches the "don't redesign
 * previous work" instruction by front-loading the one part of this file
 * that truly cannot change once written (the struct layout).
 *
 * Reference: UEFI Specification 2.10, Section 4.4 (EFI_BOOT_SERVICES),
 * Section 6 (Task Priority / Event / Timer Services), Section 7
 * (Memory/Protocol/Image Services).
 */

#ifndef OS_EFI_BOOT_SERVICES_H
#define OS_EFI_BOOT_SERVICES_H

#include "efi_types.h"
#include "efi_tables.h"

/* ---- Supporting enums and structs used by Boot Services signatures ---- */

typedef UINT64 EFI_PHYSICAL_ADDRESS;
typedef UINT64 EFI_VIRTUAL_ADDRESS;

/* EFI_ALLOCATE_TYPE — how AllocatePages should interpret the memory
 * address argument. Values per Spec Section 7.2. */
typedef enum {
    AllocateAnyPages,
    AllocateMaxAddress,
    AllocateAddress,
    MaxAllocateType
} EFI_ALLOCATE_TYPE;

/* EFI_MEMORY_TYPE — classifies a memory region's purpose. Firmware and
 * our own AllocatePages/AllocatePool calls both use these. We only list
 * the values this bootloader references directly or will reference in
 * Part 3/4; the full enum (spec defines ~16 values) is added when a
 * later part first needs a value not yet listed here. */
typedef enum {
    EfiReservedMemoryType,
    EfiLoaderCode,
    EfiLoaderData,
    EfiBootServicesCode,
    EfiBootServicesData,
    EfiRuntimeServicesCode,
    EfiRuntimeServicesData,
    EfiConventionalMemory,
    EfiUnusableMemory,
    EfiACPIReclaimMemory,
    EfiACPIMemoryNVS,
    EfiMemoryMappedIO,
    EfiMemoryMappedIOPortSpace,
    EfiPalCode,
    EfiPersistentMemory,
    EfiMaxMemoryType
} EFI_MEMORY_TYPE;

/* EFI_MEMORY_DESCRIPTOR — one entry in the memory map GetMemoryMap
 * returns. Fully defined now even though GetMemoryMap isn't called
 * until Part 3, because EFI_GET_MEMORY_MAP's signature (below)
 * references it and must be correct today. */
typedef struct {
    UINT32                Type;          /* an EFI_MEMORY_TYPE value */
    EFI_PHYSICAL_ADDRESS   PhysicalStart;
    EFI_VIRTUAL_ADDRESS    VirtualStart;
    UINT64                 NumberOfPages;
    UINT64                 Attribute;
} EFI_MEMORY_DESCRIPTOR;

typedef enum {
    TimerCancel,
    TimerPeriodic,
    TimerRelative
} EFI_TIMER_DELAY;

typedef enum {
    EFI_NATIVE_INTERFACE
} EFI_INTERFACE_TYPE;

typedef enum {
    AllHandles,
    ByRegisterNotify,
    ByProtocol
} EFI_LOCATE_SEARCH_TYPE;

/* Opaque — device path nodes are only ever passed as pointers by this
 * bootloader (through LocateDevicePath / LoadImage signatures); their
 * internal structure is not needed until the boot menu part reads and
 * displays device paths for available boot options. */
typedef struct EFI_DEVICE_PATH_PROTOCOL_STRUCT EFI_DEVICE_PATH_PROTOCOL;

typedef struct {
    EFI_HANDLE AgentHandle;
    EFI_HANDLE ControllerHandle;
    UINT32     Attributes;
    UINT32     OpenCount;
} EFI_OPEN_PROTOCOL_INFORMATION_ENTRY;

/* Generic event-notification callback signature, used by CreateEvent /
 * CreateEventEx. */
typedef VOID (EFIAPI *EFI_EVENT_NOTIFY)(
    IN EFI_EVENT Event,
    IN VOID      *Context
);

/* ---- Task Priority Services --------------------------------------- */

typedef EFI_TPL (EFIAPI *EFI_RAISE_TPL)(
    IN EFI_TPL NewTpl
);

typedef VOID (EFIAPI *EFI_RESTORE_TPL)(
    IN EFI_TPL OldTpl
);

/* ---- Memory Services ------------------------------------------------
 * AllocatePages/FreePages/GetMemoryMap are called starting Part 3
 * (memory map retrieval and ExitBootServices), AllocatePool/FreePool
 * are called starting this part (Part 2), for the file-read buffer. */

typedef EFI_STATUS (EFIAPI *EFI_ALLOCATE_PAGES)(
    IN     EFI_ALLOCATE_TYPE     Type,
    IN     EFI_MEMORY_TYPE       MemoryType,
    IN     UINTN                 Pages,
    IN OUT EFI_PHYSICAL_ADDRESS  *Memory
);

typedef EFI_STATUS (EFIAPI *EFI_FREE_PAGES)(
    IN EFI_PHYSICAL_ADDRESS Memory,
    IN UINTN                Pages
);

typedef EFI_STATUS (EFIAPI *EFI_GET_MEMORY_MAP)(
    IN OUT UINTN                  *MemoryMapSize,
    OUT    EFI_MEMORY_DESCRIPTOR  *MemoryMap,
    OUT    UINTN                  *MapKey,
    OUT    UINTN                  *DescriptorSize,
    OUT    UINT32                 *DescriptorVersion
);

typedef EFI_STATUS (EFIAPI *EFI_ALLOCATE_POOL)(
    IN  EFI_MEMORY_TYPE PoolType,
    IN  UINTN            Size,
    OUT VOID             **Buffer
);

typedef EFI_STATUS (EFIAPI *EFI_FREE_POOL)(
    IN VOID *Buffer
);

/* ---- Event & Timer Services ------------------------------------------ */

typedef EFI_STATUS (EFIAPI *EFI_CREATE_EVENT)(
    IN  UINT32            Type,
    IN  EFI_TPL            NotifyTpl,
    IN  EFI_EVENT_NOTIFY    NotifyFunction  OPTIONAL,
    IN  VOID                *NotifyContext  OPTIONAL,
    OUT EFI_EVENT            *Event
);

typedef EFI_STATUS (EFIAPI *EFI_SET_TIMER)(
    IN EFI_EVENT       Event,
    IN EFI_TIMER_DELAY  Type,
    IN UINT64            TriggerTime
);

typedef EFI_STATUS (EFIAPI *EFI_WAIT_FOR_EVENT)(
    IN  UINTN     NumberOfEvents,
    IN  EFI_EVENT *Event,
    OUT UINTN     *Index
);

typedef EFI_STATUS (EFIAPI *EFI_SIGNAL_EVENT)(
    IN EFI_EVENT Event
);

typedef EFI_STATUS (EFIAPI *EFI_CLOSE_EVENT)(
    IN EFI_EVENT Event
);

typedef EFI_STATUS (EFIAPI *EFI_CHECK_EVENT)(
    IN EFI_EVENT Event
);

/* ---- Protocol Handler Services ---------------------------------------- */

typedef EFI_STATUS (EFIAPI *EFI_INSTALL_PROTOCOL_INTERFACE)(
    IN OUT EFI_HANDLE          *Handle,
    IN     EFI_GUID            *Protocol,
    IN     EFI_INTERFACE_TYPE  InterfaceType,
    IN     VOID                *Interface
);

typedef EFI_STATUS (EFIAPI *EFI_REINSTALL_PROTOCOL_INTERFACE)(
    IN EFI_HANDLE Handle,
    IN EFI_GUID   *Protocol,
    IN VOID       *OldInterface,
    IN VOID       *NewInterface
);

typedef EFI_STATUS (EFIAPI *EFI_UNINSTALL_PROTOCOL_INTERFACE)(
    IN EFI_HANDLE Handle,
    IN EFI_GUID   *Protocol,
    IN VOID       *Interface
);

typedef EFI_STATUS (EFIAPI *EFI_HANDLE_PROTOCOL)(
    IN  EFI_HANDLE Handle,
    IN  EFI_GUID   *Protocol,
    OUT VOID       **Interface
);

typedef EFI_STATUS (EFIAPI *EFI_REGISTER_PROTOCOL_NOTIFY)(
    IN  EFI_GUID  *Protocol,
    IN  EFI_EVENT Event,
    OUT VOID      **Registration
);

typedef EFI_STATUS (EFIAPI *EFI_LOCATE_HANDLE)(
    IN     EFI_LOCATE_SEARCH_TYPE SearchType,
    IN     EFI_GUID               *Protocol       OPTIONAL,
    IN     VOID                   *SearchKey      OPTIONAL,
    IN OUT UINTN                  *BufferSize,
    OUT    EFI_HANDLE             *Buffer
);

typedef EFI_STATUS (EFIAPI *EFI_LOCATE_DEVICE_PATH)(
    IN     EFI_GUID                  *Protocol,
    IN OUT EFI_DEVICE_PATH_PROTOCOL  **DevicePath,
    OUT    EFI_HANDLE                *Device
);

typedef EFI_STATUS (EFIAPI *EFI_INSTALL_CONFIGURATION_TABLE)(
    IN EFI_GUID *Guid,
    IN VOID     *Table
);

/* ---- Image Services -----------------------------------------------------
 * LoadImage/StartImage exist in the table (required for correct layout)
 * but this bootloader does not call them: we hand off to the kernel by
 * loading its raw binary ourselves via the file protocol and jumping to
 * its entry point directly (Part 4), rather than asking firmware to load
 * it as a second UEFI application — the kernel is not a UEFI application
 * and has no PE headers for LoadImage to parse. */

typedef EFI_STATUS (EFIAPI *EFI_IMAGE_LOAD)(
    IN  BOOLEAN                   BootPolicy,
    IN  EFI_HANDLE                 ParentImageHandle,
    IN  EFI_DEVICE_PATH_PROTOCOL   *DevicePath   OPTIONAL,
    IN  VOID                       *SourceBuffer OPTIONAL,
    IN  UINTN                      SourceSize,
    OUT EFI_HANDLE                 *ImageHandle
);

typedef EFI_STATUS (EFIAPI *EFI_IMAGE_START)(
    IN  EFI_HANDLE  ImageHandle,
    OUT UINTN       *ExitDataSize,
    OUT CHAR16      **ExitData OPTIONAL
);

typedef EFI_STATUS (EFIAPI *EFI_EXIT)(
    IN EFI_HANDLE ImageHandle,
    IN EFI_STATUS ExitStatus,
    IN UINTN      ExitDataSize,
    IN CHAR16     *ExitData OPTIONAL
);

typedef EFI_STATUS (EFIAPI *EFI_IMAGE_UNLOAD)(
    IN EFI_HANDLE ImageHandle
);

/* ExitBootServices — the single most consequential call this bootloader
 * will ever make (Part 3). Defined accurately now for struct-layout
 * correctness. */
typedef EFI_STATUS (EFIAPI *EFI_EXIT_BOOT_SERVICES)(
    IN EFI_HANDLE ImageHandle,
    IN UINTN      MapKey
);

/* ---- Miscellaneous Services --------------------------------------------- */

typedef EFI_STATUS (EFIAPI *EFI_GET_NEXT_MONOTONIC_COUNT)(
    OUT UINT64 *Count
);

typedef EFI_STATUS (EFIAPI *EFI_STALL)(
    IN UINTN Microseconds
);

typedef EFI_STATUS (EFIAPI *EFI_SET_WATCHDOG_TIMER)(
    IN UINTN   Timeout,
    IN UINT64  WatchdogCode,
    IN UINTN   DataSize,
    IN CHAR16  *WatchdogData OPTIONAL
);

/* ---- DriverSupport Services ------------------------------------------- */

typedef EFI_STATUS (EFIAPI *EFI_CONNECT_CONTROLLER)(
    IN EFI_HANDLE                  ControllerHandle,
    IN EFI_HANDLE                  *DriverImageHandle    OPTIONAL,
    IN EFI_DEVICE_PATH_PROTOCOL    *RemainingDevicePath  OPTIONAL,
    IN BOOLEAN                     Recursive
);

typedef EFI_STATUS (EFIAPI *EFI_DISCONNECT_CONTROLLER)(
    IN EFI_HANDLE ControllerHandle,
    IN EFI_HANDLE DriverImageHandle    OPTIONAL,
    IN EFI_HANDLE ChildHandle          OPTIONAL
);

/* ---- Open and Close Protocol Services -----------------------------------
 * OpenProtocol/CloseProtocol are how this bootloader will actually
 * acquire the Loaded Image and Simple File System protocol interfaces
 * in Part 2's main.c (preferred over the legacy HandleProtocol per
 * spec guidance, though HandleProtocol is also defined above for
 * completeness of the table). */

typedef EFI_STATUS (EFIAPI *EFI_OPEN_PROTOCOL)(
    IN  EFI_HANDLE  Handle,
    IN  EFI_GUID    *Protocol,
    OUT VOID        **Interface OPTIONAL,
    IN  EFI_HANDLE  AgentHandle,
    IN  EFI_HANDLE  ControllerHandle,
    IN  UINT32      Attributes
);

typedef EFI_STATUS (EFIAPI *EFI_CLOSE_PROTOCOL)(
    IN EFI_HANDLE Handle,
    IN EFI_GUID   *Protocol,
    IN EFI_HANDLE AgentHandle,
    IN EFI_HANDLE ControllerHandle
);

typedef EFI_STATUS (EFIAPI *EFI_OPEN_PROTOCOL_INFORMATION)(
    IN  EFI_HANDLE                            Handle,
    IN  EFI_GUID                              *Protocol,
    OUT EFI_OPEN_PROTOCOL_INFORMATION_ENTRY   **EntryBuffer,
    OUT UINTN                                 *EntryCount
);

/* ---- Library Services ---------------------------------------------------
 * LocateProtocol is the second protocol-acquisition call main.c uses
 * this part (for the Simple File System Protocol, which is looked up
 * by GUID directly rather than off a specific handle). */

typedef EFI_STATUS (EFIAPI *EFI_PROTOCOLS_PER_HANDLE)(
    IN  EFI_HANDLE Handle,
    OUT EFI_GUID   ***ProtocolBuffer,
    OUT UINTN      *ProtocolBufferCount
);

typedef EFI_STATUS (EFIAPI *EFI_LOCATE_HANDLE_BUFFER)(
    IN     EFI_LOCATE_SEARCH_TYPE SearchType,
    IN     EFI_GUID               *Protocol    OPTIONAL,
    IN     VOID                   *SearchKey   OPTIONAL,
    OUT    UINTN                  *NoHandles,
    OUT    EFI_HANDLE             **Buffer
);

typedef EFI_STATUS (EFIAPI *EFI_LOCATE_PROTOCOL)(
    IN  EFI_GUID  *Protocol,
    IN  VOID      *Registration  OPTIONAL,
    OUT VOID      **Interface
);

typedef EFI_STATUS (EFIAPI *EFI_INSTALL_MULTIPLE_PROTOCOL_INTERFACES)(
    IN OUT EFI_HANDLE *Handle,
    ...
);

typedef EFI_STATUS (EFIAPI *EFI_UNINSTALL_MULTIPLE_PROTOCOL_INTERFACES)(
    IN EFI_HANDLE Handle,
    ...
);

/* ---- 32-bit CRC Services -------------------------------------------------- */

typedef EFI_STATUS (EFIAPI *EFI_CALCULATE_CRC32)(
    IN  VOID    *Data,
    IN  UINTN   DataSize,
    OUT UINT32  *Crc32
);

/* ---- Misc Services ------------------------------------------------------ */

typedef VOID (EFIAPI *EFI_COPY_MEM)(
    IN VOID  *Destination,
    IN VOID  *Source,
    IN UINTN Length
);

typedef VOID (EFIAPI *EFI_SET_MEM)(
    IN VOID  *Buffer,
    IN UINTN Size,
    IN UINT8 Value
);

/* ---- Advanced Configuration Services -------------------------------------- */

typedef EFI_STATUS (EFIAPI *EFI_CREATE_EVENT_EX)(
    IN  UINT32            Type,
    IN  EFI_TPL           NotifyTpl,
    IN  EFI_EVENT_NOTIFY  NotifyFunction  OPTIONAL,
    IN  const VOID        *NotifyContext  OPTIONAL,
    IN  const EFI_GUID    *EventGroup     OPTIONAL,
    OUT EFI_EVENT         *Event
);

/* ---- The table itself ----------------------------------------------------
 * Field order below is the ENTIRE point of this file — see header
 * comment. Do not reorder, insert, or remove fields without re-deriving
 * this from the spec table again from scratch. */
struct EFI_BOOT_SERVICES_STRUCT {
    EFI_TABLE_HEADER Hdr;

    EFI_RAISE_TPL   RaiseTPL;
    EFI_RESTORE_TPL RestoreTPL;

    EFI_ALLOCATE_PAGES AllocatePages;
    EFI_FREE_PAGES     FreePages;
    EFI_GET_MEMORY_MAP GetMemoryMap;
    EFI_ALLOCATE_POOL  AllocatePool;
    EFI_FREE_POOL      FreePool;

    EFI_CREATE_EVENT   CreateEvent;
    EFI_SET_TIMER      SetTimer;
    EFI_WAIT_FOR_EVENT WaitForEvent;
    EFI_SIGNAL_EVENT   SignalEvent;
    EFI_CLOSE_EVENT    CloseEvent;
    EFI_CHECK_EVENT    CheckEvent;

    EFI_INSTALL_PROTOCOL_INTERFACE   InstallProtocolInterface;
    EFI_REINSTALL_PROTOCOL_INTERFACE ReinstallProtocolInterface;
    EFI_UNINSTALL_PROTOCOL_INTERFACE UninstallProtocolInterface;
    EFI_HANDLE_PROTOCOL              HandleProtocol;
    VOID                              *Reserved;
    EFI_REGISTER_PROTOCOL_NOTIFY     RegisterProtocolNotify;
    EFI_LOCATE_HANDLE                LocateHandle;
    EFI_LOCATE_DEVICE_PATH           LocateDevicePath;
    EFI_INSTALL_CONFIGURATION_TABLE  InstallConfigurationTable;

    EFI_IMAGE_LOAD          LoadImage;
    EFI_IMAGE_START         StartImage;
    EFI_EXIT                Exit;
    EFI_IMAGE_UNLOAD        UnloadImage;
    EFI_EXIT_BOOT_SERVICES  ExitBootServices;

    EFI_GET_NEXT_MONOTONIC_COUNT GetNextMonotonicCount;
    EFI_STALL                    Stall;
    EFI_SET_WATCHDOG_TIMER       SetWatchdogTimer;

    EFI_CONNECT_CONTROLLER    ConnectController;
    EFI_DISCONNECT_CONTROLLER DisconnectController;

    EFI_OPEN_PROTOCOL             OpenProtocol;
    EFI_CLOSE_PROTOCOL            CloseProtocol;
    EFI_OPEN_PROTOCOL_INFORMATION OpenProtocolInformation;

    EFI_PROTOCOLS_PER_HANDLE                   ProtocolsPerHandle;
    EFI_LOCATE_HANDLE_BUFFER                   LocateHandleBuffer;
    EFI_LOCATE_PROTOCOL                        LocateProtocol;
    EFI_INSTALL_MULTIPLE_PROTOCOL_INTERFACES   InstallMultipleProtocolInterfaces;
    EFI_UNINSTALL_MULTIPLE_PROTOCOL_INTERFACES UninstallMultipleProtocolInterfaces;

    EFI_CALCULATE_CRC32 CalculateCrc32;

    EFI_COPY_MEM CopyMem;
    EFI_SET_MEM  SetMem;

    EFI_CREATE_EVENT_EX CreateEventEx;
};

#endif /* OS_EFI_BOOT_SERVICES_H */
