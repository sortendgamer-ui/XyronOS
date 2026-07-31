/*
 * efi_tables.h — UEFI System Table and the console protocols our
 * Part 1 entry point uses.
 *
 * Struct field ORDER here is not a design choice — it is dictated by
 * the UEFI specification, because firmware lays these structs out in
 * memory exactly this way and hands us a raw pointer to them. Every
 * field, including ones we never touch, must appear in the correct
 * position or every field after it reads garbage.
 *
 * Reference: UEFI Specification 2.10, Section 4.2 (EFI_TABLE_HEADER),
 * Section 4.3 (EFI_SYSTEM_TABLE), Section 12.4 (Simple Text Output).
 */

#ifndef OS_EFI_TABLES_H
#define OS_EFI_TABLES_H

#include "efi_types.h"

/* IN/OUT/OPTIONAL are pure documentation markers used throughout the
 * UEFI spec's own function prototypes — they expand to nothing, but
 * having them lets every signature in this codebase read identically
 * to the spec text it was transcribed from, which matters when a
 * future ADR or bug report needs to cross-reference back to it. */
#define IN
#define OUT
#define OPTIONAL

/* ---- Common table header ----------------------------------------------
 * Every top-level UEFI table (System Table, Boot Services, Runtime
 * Services) starts with this same header, which is how firmware lets
 * us verify we're looking at a table of the type/revision we expect. */
typedef struct {
    UINT64 Signature;
    UINT32 Revision;
    UINT32 HeaderSize;
    UINT32 CRC32;
    UINT32 Reserved;
} EFI_TABLE_HEADER;

/* Forward declarations only — full function tables are defined in
 * Phase 2 Part 2 (efi_boot_services.h) and a later part
 * (efi_runtime_services.h) when this bootloader first calls into them.
 * EFI_SYSTEM_TABLE only needs to hold *pointers* to these types, and a
 * pointer's size does not depend on whether the pointee type is fully
 * defined yet. */
typedef struct EFI_BOOT_SERVICES_STRUCT    EFI_BOOT_SERVICES;
typedef struct EFI_RUNTIME_SERVICES_STRUCT EFI_RUNTIME_SERVICES;

/* ---- Simple Text Output Protocol ---------------------------------------
 * GUID and interface for printing to the console. This is the one
 * protocol Part 1 actually calls, so it is fully defined now. */

#define EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL_GUID \
    { 0x387477c2, 0x69c7, 0x11d2, \
      { 0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b } }

typedef struct EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL_STRUCT
    EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL;

typedef EFI_STATUS (EFIAPI *EFI_TEXT_RESET)(
    IN EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *This,
    IN BOOLEAN ExtendedVerification
);

typedef EFI_STATUS (EFIAPI *EFI_TEXT_STRING)(
    IN EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *This,
    IN CHAR16 *String
);

typedef EFI_STATUS (EFIAPI *EFI_TEXT_TEST_STRING)(
    IN EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *This,
    IN CHAR16 *String
);

typedef EFI_STATUS (EFIAPI *EFI_TEXT_QUERY_MODE)(
    IN  EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *This,
    IN  UINTN ModeNumber,
    OUT UINTN *Columns,
    OUT UINTN *Rows
);

typedef EFI_STATUS (EFIAPI *EFI_TEXT_SET_MODE)(
    IN EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *This,
    IN UINTN ModeNumber
);

typedef EFI_STATUS (EFIAPI *EFI_TEXT_SET_ATTRIBUTE)(
    IN EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *This,
    IN UINTN Attribute
);

typedef EFI_STATUS (EFIAPI *EFI_TEXT_CLEAR_SCREEN)(
    IN EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *This
);

typedef EFI_STATUS (EFIAPI *EFI_TEXT_SET_CURSOR_POSITION)(
    IN EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *This,
    IN UINTN Column,
    IN UINTN Row
);

typedef EFI_STATUS (EFIAPI *EFI_TEXT_ENABLE_CURSOR)(
    IN EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *This,
    IN BOOLEAN Visible
);

/* Mode state block — firmware owns this memory, we only read it. */
typedef struct {
    INT32   MaxMode;
    INT32   Mode;
    INT32   Attribute;
    INT32   CursorColumn;
    INT32   CursorRow;
    BOOLEAN CursorVisible;
} SIMPLE_TEXT_OUTPUT_MODE;

struct EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL_STRUCT {
    EFI_TEXT_RESET               Reset;
    EFI_TEXT_STRING              OutputString;
    EFI_TEXT_TEST_STRING         TestString;
    EFI_TEXT_QUERY_MODE          QueryMode;
    EFI_TEXT_SET_MODE            SetMode;
    EFI_TEXT_SET_ATTRIBUTE       SetAttribute;
    EFI_TEXT_CLEAR_SCREEN        ClearScreen;
    EFI_TEXT_SET_CURSOR_POSITION SetCursorPosition;
    EFI_TEXT_ENABLE_CURSOR       EnableCursor;
    SIMPLE_TEXT_OUTPUT_MODE      *Mode;
};

/* Simple Text Input — only forward-declared. EFI_SYSTEM_TABLE holds a
 * pointer to it (ConIn) that Part 1 never dereferences; full definition
 * arrives whenever we first read keyboard input (boot menu, later in
 * Phase 2). */
typedef struct EFI_SIMPLE_TEXT_INPUT_PROTOCOL_STRUCT
    EFI_SIMPLE_TEXT_INPUT_PROTOCOL;

/* ---- EFI_SYSTEM_TABLE ---------------------------------------------------
 * The one struct firmware guarantees is fully valid the moment our
 * entry point runs. Field order per UEFI Spec Section 4.3, Table 4.5. */
typedef struct {
    EFI_TABLE_HEADER Hdr;

    CHAR16 *FirmwareVendor;
    UINT32  FirmwareRevision;

    EFI_HANDLE ConsoleInHandle;
    EFI_SIMPLE_TEXT_INPUT_PROTOCOL *ConIn;

    EFI_HANDLE ConsoleOutHandle;
    EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *ConOut;

    EFI_HANDLE StandardErrorHandle;
    EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL *StdErr;

    EFI_RUNTIME_SERVICES *RuntimeServices;
    EFI_BOOT_SERVICES    *BootServices;

    UINTN NumberOfTableEntries;
    VOID  *ConfigurationTable; /* EFI_CONFIGURATION_TABLE*, defined when
                                  first needed (ACPI table lookup). */
} EFI_SYSTEM_TABLE;

#endif /* OS_EFI_TABLES_H */
