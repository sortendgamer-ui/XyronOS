/*
 * efi_types.h — Fundamental UEFI data types.
 *
 * The UEFI specification defines its own fixed-width scalar types instead
 * of relying on C's `int`/`long`/etc., because those widths are not
 * portable across compilers or architectures. Every struct and function
 * signature in this bootloader is built from these types, so getting
 * them wrong here would silently corrupt every struct layout that follows.
 *
 * Reference: UEFI Specification 2.10, Section 2.3 ("Data Types").
 */

#ifndef OS_EFI_TYPES_H
#define OS_EFI_TYPES_H

/* ---- Fixed-width integers -------------------------------------------- */
/* We deliberately do NOT include <stdint.h> — this is freestanding code
 * with no guarantee a hosted C library's headers are even present, and
 * the whole point of this file is to pin down the types ourselves. */

typedef unsigned char      UINT8;
typedef signed char        INT8;
typedef unsigned short      UINT16;
typedef signed short        INT16;
typedef unsigned int        UINT32;
typedef signed int          INT32;
typedef unsigned long long  UINT64;
typedef signed long long    INT64;

/* UINTN/INTN are "natural word size" integers — 8 bytes on the x86_64
 * target we committed to in ADR-002. If we ever add a 32-bit target,
 * these must be redefined per-architecture. */
typedef UINT64 UINTN;
typedef INT64  INTN;

typedef UINT8   BOOLEAN;
#define TRUE  1
#define FALSE 0

typedef void VOID;

/* UEFI strings are UCS-2 (2-byte, not null-narrow like ASCII, not full
 * UTF-16 with surrogate pairs). CHAR16 is exactly the mingw-w64 wchar_t
 * width for our chosen toolchain, but we define it explicitly rather
 * than depending on that coincidence. */
typedef UINT16 CHAR16;
typedef char   CHAR8;

/* ---- Calling convention ------------------------------------------------
 * UEFI mandates the Microsoft x64 calling convention (rcx/rdx/r8/r9,
 * caller-allocated shadow space) for every function it calls or is
 * called by, regardless of host platform. Building with a mingw-w64
 * target compiler makes this the default ABI, so EFIAPI is a no-op
 * marker here — but every function pointer typedef in efi_tables.h
 * carries it so the convention is documented at every call site and
 * the code stays correct if we ever cross-compile with a non-Windows-
 * targeting compiler that would otherwise default to System V ABI. */
#define EFIAPI

/* ---- Status codes -------------------------------------------------------
 * EFI_STATUS is UINTN. The high bit (bit 63 on x86_64) set means error;
 * clear means success or a warning. EFI_SUCCESS is always zero. */
typedef UINTN EFI_STATUS;

#define EFI_ERROR_BIT   (((UINTN)1) << 63)
#define EFI_SUCCESS     ((EFI_STATUS)0)
#define EFI_ERROR(x)    (((INTN)(x)) < 0)

/* Only the codes this bootloader currently checks. More are added to
 * this list as later parts of Phase 2 call functions that return them —
 * we do not pre-declare status codes we have no code path for yet. */
#define EFI_LOAD_ERROR         (EFI_ERROR_BIT | 1)
#define EFI_INVALID_PARAMETER  (EFI_ERROR_BIT | 2)
#define EFI_NOT_FOUND          (EFI_ERROR_BIT | 14)
#define EFI_BUFFER_TOO_SMALL   (EFI_ERROR_BIT | 5)

/* ---- Handles and GUIDs -------------------------------------------------- */

/* An EFI_HANDLE is an opaque pointer — firmware defines what it points
 * to internally; we only ever pass it back to firmware, never dereference
 * it ourselves. */
typedef VOID* EFI_HANDLE;

/* Opaque event handle, used by timer/event-notification services. */
typedef VOID* EFI_EVENT;

typedef UINTN EFI_TPL;

/* A GUID uniquely identifies a protocol, a variable namespace, or a
 * configuration table. Layout is mandated by the spec (Section 2.3.1):
 * 32-bit, 16-bit, 16-bit, then 8 bytes — NOT four UINT32s, because the
 * third and fourth fields are only 16 bits wide each. Getting this
 * field width wrong would misalign every GUID comparison against
 * firmware-provided data. */
typedef struct {
    UINT32 Data1;
    UINT16 Data2;
    UINT16 Data3;
    UINT8  Data4[8];
} EFI_GUID;

#endif /* OS_EFI_TYPES_H */
