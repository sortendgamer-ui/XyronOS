/*
 * elf.h — ELF64 structures needed to parse and load an executable.
 *
 * Field layout here is dictated by the ELF specification (Tool
 * Interface Standard, Portable Formats Specification) and the System
 * V AMD64 ABI supplement's machine-specific values (e.g. EM_X86_64).
 * This is an external, documented file format specification — the
 * same category as UEFI's own structures — not source taken from an
 * existing loader implementation. Only the subset of ELF64 actually
 * needed to load a static ET_EXEC executable is defined; dynamic
 * linking structures (dynamic section, relocations, symbol tables)
 * are intentionally omitted since ADR-005 restricts this bootloader
 * to ET_EXEC images.
 */

#ifndef OS_ELF_H
#define OS_ELF_H

#include "efi_types.h"

typedef UINT64 Elf64_Addr;
typedef UINT64 Elf64_Off;
typedef UINT16 Elf64_Half;
typedef UINT32 Elf64_Word;
typedef INT32  Elf64_Sword;
typedef UINT64 Elf64_Xword;
typedef INT64  Elf64_Sxword;

#define EI_NIDENT 16

/* e_ident[] indices */
#define EI_MAG0       0
#define EI_MAG1       1
#define EI_MAG2       2
#define EI_MAG3       3
#define EI_CLASS      4
#define EI_DATA       5
#define EI_VERSION    6
#define EI_OSABI      7

#define ELFMAG0 0x7F
#define ELFMAG1 'E'
#define ELFMAG2 'L'
#define ELFMAG3 'F'

#define ELFCLASS64  2
#define ELFDATA2LSB 1 /* little-endian, what x86_64 uses */

#define ET_EXEC 2   /* the only type this bootloader accepts, per ADR-005 */
#define EM_X86_64 62

#define PT_LOAD 1   /* the only segment type this bootloader loads */

typedef struct {
    UINT8      e_ident[EI_NIDENT];
    Elf64_Half e_type;
    Elf64_Half e_machine;
    Elf64_Word e_version;
    Elf64_Addr e_entry;      /* virtual address of the entry point */
    Elf64_Off  e_phoff;      /* file offset of the program header table */
    Elf64_Off  e_shoff;
    Elf64_Word e_flags;
    Elf64_Half e_ehsize;
    Elf64_Half e_phentsize;  /* size of one program header entry */
    Elf64_Half e_phnum;      /* number of program header entries */
    Elf64_Half e_shentsize;
    Elf64_Half e_shnum;
    Elf64_Half e_shstrndx;
} Elf64_Ehdr;

typedef struct {
    Elf64_Word  p_type;
    Elf64_Word  p_flags;
    Elf64_Off   p_offset;   /* file offset of this segment's data */
    Elf64_Addr  p_vaddr;    /* virtual address this segment loads to */
    Elf64_Addr  p_paddr;    /* physical address hint — unused; we
                                choose physical placement ourselves */
    Elf64_Xword p_filesz;   /* bytes of this segment present in the file */
    Elf64_Xword p_memsz;    /* bytes this segment occupies in memory —
                                >= p_filesz; the difference is BSS and
                                must be zero-filled, not copied */
    Elf64_Xword p_align;
} Elf64_Phdr;

#endif /* OS_ELF_H */
