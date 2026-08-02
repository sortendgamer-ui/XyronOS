/*
 * linker.ld — places the kernel at KERNEL_VIRTUAL_BASE
 * (0xFFFFFFFF80000000, ADR-002/ADR-005), entry point kernel_main.
 * Same structure as tests/kernel_stub/linker.ld, since both target
 * the same handoff contract — this is the real kernel's counterpart,
 * not a copy kept in sync by hand for any other reason.
 */

ENTRY(kernel_main)

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
