/*
 * linker.ld — places the kernel stub at KERNEL_VIRTUAL_BASE
 * (0xFFFFFFFF80000000, per ADR-002/ADR-005) as a single ET_EXEC,
 * non-PIE ELF64 image, entry point KernelEntry.
 */

ENTRY(KernelEntry)

SECTIONS
{
    . = 0xFFFFFFFF80000000;

    .text : {
        *(.text*)
    }

    .rodata : {
        *(.rodata*)
    }

    .data : {
        *(.data*)
    }

    .bss : {
        *(COMMON)
        *(.bss*)
    }

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame)
        *(.note*)
    }
}
