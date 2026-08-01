/*
 * efi_file_protocol.h — EFI_SIMPLE_FILE_SYSTEM_PROTOCOL and
 * EFI_FILE_PROTOCOL.
 *
 * This is how the bootloader reads files (currently a test file to
 * prove the pipeline; the kernel image itself starting Part 4) off the
 * ESP without writing our own FAT12/16/32 parser. Firmware already
 * ships a Simple File System driver over its Block I/O driver for the
 * boot volume — using it through this standard protocol interface is
 * calling a documented UEFI service, the same category as calling
 * AllocatePages, not "borrowing bootloader source."
 *
 * Reference: UEFI Specification 2.10, Section 13.4
 * (EFI_SIMPLE_FILE_SYSTEM_PROTOCOL), Section 13.5 (EFI_FILE_PROTOCOL).
 */

#ifndef OS_EFI_FILE_PROTOCOL_H
#define OS_EFI_FILE_PROTOCOL_H

#include "efi_types.h"

#define EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_GUID \
    { 0x964e5b22, 0x6459, 0x11d2, \
      { 0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b } }

#define EFI_FILE_INFO_ID \
    { 0x09576e92, 0x6d3f, 0x11d2, \
      { 0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b } }

/* Open() Mode values — Section 13.5. */
#define EFI_FILE_MODE_READ   0x0000000000000001ULL
#define EFI_FILE_MODE_WRITE  0x0000000000000002ULL
#define EFI_FILE_MODE_CREATE 0x8000000000000000ULL

/* Open() Attributes values, used only when creating a file (not by
 * this bootloader, which only ever reads — kept for struct/API
 * completeness since Open()'s signature includes the parameter
 * regardless). */
#define EFI_FILE_READ_ONLY 0x0000000000000001ULL
#define EFI_FILE_HIDDEN    0x0000000000000002ULL
#define EFI_FILE_SYSTEM    0x0000000000000004ULL
#define EFI_FILE_DIRECTORY 0x0000000000000010ULL
#define EFI_FILE_ARCHIVE   0x0000000000000020ULL

typedef struct EFI_FILE_PROTOCOL_STRUCT EFI_FILE_PROTOCOL;

typedef struct {
    UINT16 Year;
    UINT8  Month;
    UINT8  Day;
    UINT8  Hour;
    UINT8  Minute;
    UINT8  Second;
    UINT8  Pad1;
    UINT32 Nanosecond;
    INT16  TimeZone;
    UINT8  Daylight;
    UINT8  Pad2;
} EFI_TIME;

/* EFI_FILE_INFO — returned by GetInfo() when queried with
 * EFI_FILE_INFO_ID. We need FileSize to know how large a buffer to
 * allocate before reading a file's full contents. FileName is a
 * variable-length trailing CHAR16 array per spec — this struct's
 * *declared* size is therefore only the fixed-size prefix; callers
 * must allocate GetInfo()'s reported buffer size, not sizeof(this
 * struct), exactly as main.c does. */
typedef struct {
    UINT64   Size;
    UINT64   FileSize;
    UINT64   PhysicalSize;
    EFI_TIME CreateTime;
    EFI_TIME LastAccessTime;
    EFI_TIME ModificationTime;
    UINT64   Attribute;
    CHAR16   FileName[1]; /* variable-length in practice */
} EFI_FILE_INFO;

typedef EFI_STATUS (EFIAPI *EFI_FILE_OPEN)(
    IN  EFI_FILE_PROTOCOL *This,
    OUT EFI_FILE_PROTOCOL **NewHandle,
    IN  CHAR16             *FileName,
    IN  UINT64              OpenMode,
    IN  UINT64              Attributes
);

typedef EFI_STATUS (EFIAPI *EFI_FILE_CLOSE)(
    IN EFI_FILE_PROTOCOL *This
);

typedef EFI_STATUS (EFIAPI *EFI_FILE_DELETE)(
    IN EFI_FILE_PROTOCOL *This
);

typedef EFI_STATUS (EFIAPI *EFI_FILE_READ)(
    IN     EFI_FILE_PROTOCOL *This,
    IN OUT UINTN              *BufferSize,
    OUT    VOID               *Buffer
);

typedef EFI_STATUS (EFIAPI *EFI_FILE_WRITE)(
    IN     EFI_FILE_PROTOCOL *This,
    IN OUT UINTN              *BufferSize,
    IN     VOID               *Buffer
);

typedef EFI_STATUS (EFIAPI *EFI_FILE_GET_POSITION)(
    IN  EFI_FILE_PROTOCOL *This,
    OUT UINT64             *Position
);

typedef EFI_STATUS (EFIAPI *EFI_FILE_SET_POSITION)(
    IN EFI_FILE_PROTOCOL *This,
    IN UINT64             Position
);

typedef EFI_STATUS (EFIAPI *EFI_FILE_GET_INFO)(
    IN     EFI_FILE_PROTOCOL *This,
    IN     EFI_GUID           *InformationType,
    IN OUT UINTN              *BufferSize,
    OUT    VOID               *Buffer
);

typedef EFI_STATUS (EFIAPI *EFI_FILE_SET_INFO)(
    IN EFI_FILE_PROTOCOL *This,
    IN EFI_GUID           *InformationType,
    IN UINTN               BufferSize,
    IN VOID               *Buffer
);

typedef EFI_STATUS (EFIAPI *EFI_FILE_FLUSH)(
    IN EFI_FILE_PROTOCOL *This
);

/* Revision 2 (EFI_FILE_PROTOCOL_REVISION2) adds async Ex variants.
 * Not declared: we never call them, and this bootloader only ever
 * requests/opens Simple File System at its base revision. The struct
 * below (Open..Flush) is the complete revision-1 EFI_FILE_PROTOCOL,
 * sufficient for a synchronous read-only bootloader. */

struct EFI_FILE_PROTOCOL_STRUCT {
    UINT64 Revision;

    EFI_FILE_OPEN         Open;
    EFI_FILE_CLOSE        Close;
    EFI_FILE_DELETE       Delete;
    EFI_FILE_READ         Read;
    EFI_FILE_WRITE        Write;
    EFI_FILE_GET_POSITION GetPosition;
    EFI_FILE_SET_POSITION SetPosition;
    EFI_FILE_GET_INFO     GetInfo;
    EFI_FILE_SET_INFO     SetInfo;
    EFI_FILE_FLUSH        Flush;
};

typedef struct EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_STRUCT
    EFI_SIMPLE_FILE_SYSTEM_PROTOCOL;

typedef EFI_STATUS (EFIAPI *EFI_FILE_OPEN_VOLUME)(
    IN  EFI_SIMPLE_FILE_SYSTEM_PROTOCOL *This,
    OUT EFI_FILE_PROTOCOL               **Root
);

struct EFI_SIMPLE_FILE_SYSTEM_PROTOCOL_STRUCT {
    UINT64                Revision;
    EFI_FILE_OPEN_VOLUME  OpenVolume;
};

#endif /* OS_EFI_FILE_PROTOCOL_H */
