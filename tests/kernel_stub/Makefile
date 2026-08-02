# Makefile — Kernel stub test fixture (Phase 2 Part 4). See README.md
# for why this uses the host's native gcc in freestanding mode rather
# than a dedicated x86_64-elf cross-compiler.

CC := gcc

# -ffreestanding -nostdlib      : no libc, no CRT startup — same
#                                 reasoning as the bootloader's own
#                                 Makefile, just for ELF instead of PE.
# -fno-pic -fno-pie -no-pie     : force a non-relocatable ET_EXEC
#                                 image — kernel_loader.c only accepts
#                                 ET_EXEC (ADR-005); the host gcc
#                                 defaults to producing PIE (ET_DYN)
#                                 executables, which must be overridden.
# -mcmodel=kernel                : correct code model for code linked
#                                 into the top 2 GiB of the address
#                                 space (KERNEL_VIRTUAL_BASE,
#                                 0xFFFFFFFF80000000) — matches
#                                 ADR-002's virtual memory layout
#                                 exactly.
# -mno-red-zone                  : same reasoning as the bootloader —
#                                 this code can be interrupted (or, for
#                                 the real Phase 3 kernel, will
#                                 eventually run with interrupts
#                                 enabled) with no guarantee the red
#                                 zone below RSP is safe to use.
# -fno-stack-protector            : no __stack_chk_fail runtime exists.
CFLAGS := -ffreestanding -nostdlib -fno-pic -fno-pie -mcmodel=kernel \
          -mno-red-zone -fno-stack-protector -std=c11 -Wall -Wextra \
          -I../../boot/include -c

LDFLAGS := -ffreestanding -nostdlib -no-pie -T linker.ld -Wl,--build-id=none

TARGET := kernel_stub.elf

.PHONY: all clean

all: $(TARGET)

kernel_stub.o: kernel_stub.c ../../boot/include/boot_info.h ../../boot/include/serial.h
	$(CC) $(CFLAGS) kernel_stub.c -o kernel_stub.o

# Reuses the bootloader's own serial.c (see README.md: this is a test
# fixture reusing bootloader-authored code for convenience, not a
# statement that the real Phase 3 kernel will link against boot/ code —
# ADR-004 already documents that the kernel's eventual driver is
# separate).
serial.o: ../../boot/serial.c ../../boot/include/serial.h
	$(CC) $(CFLAGS) ../../boot/serial.c -o serial.o

$(TARGET): kernel_stub.o serial.o
	$(CC) $(LDFLAGS) kernel_stub.o serial.o -o $(TARGET)
	@echo "Built $(TARGET)"

clean:
	rm -f *.o $(TARGET)
