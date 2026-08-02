# Makefile — Bootloader (Phase 2, Part 1)
#
# UEFI applications are PE32+ binaries, not ELF, regardless of what OS
# builds them. We use the mingw-w64 cross-compiler purely as a
# freestanding PE32+ code generator — none of Windows' C runtime, CRT
# startup, or libc is linked in. This is a compiler/linker TARGET choice,
# not borrowed bootloader source, consistent with the no-existing-OS-code
# rule: nothing here comes from an existing bootloader project.

CC      := x86_64-w64-mingw32-gcc
LD      := x86_64-w64-mingw32-gcc

# -ffreestanding      : no hosted-environment assumptions (no libc, no main())
# -fshort-wchar       : force 2-byte wchar_t (matches mingw default, made
#                       explicit rather than relied upon implicitly)
# -mno-red-zone       : firmware may use interrupts asynchronously to our
#                       code; the System V red zone optimization is unsafe
#                       here even though we're using MS ABI, since some
#                       codegen paths still assume a scratch area below RSP
# -fno-stack-protector: stack canaries need runtime support (__stack_chk_fail)
#                       that does not exist in a freestanding binary
# -Wall -Wextra       : every phase compiles clean, no suppressed warnings
CFLAGS := -ffreestanding -fshort-wchar -mno-red-zone -fno-stack-protector \
          -std=c11 -Wall -Wextra -Iinclude -c

# -nostdlib            : do not link any C runtime/startup object files
# --subsystem,10        : PE subsystem 10 = EFI_APPLICATION (UEFI spec's
#                         PE/COFF subsystem value for a UEFI app)
# --entry,EfiMain        : PE entry point symbol = our EfiMain function
# -e EfiMain             : (redundant with --entry, kept for clarity across
#                          linker versions that prefer one flag name)
LDFLAGS := -nostdlib -Wl,--subsystem,10 -Wl,--entry,EfiMain

BUILD_DIR := ../build
TARGET    := $(BUILD_DIR)/BOOTX64.EFI

.PHONY: all clean

all: $(TARGET)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

AS      := nasm
# -f win64: NASM's PE/COFF object format for x86_64, compatible with
# the mingw-w64 linker used for the rest of this bootloader — keeps
# trampoline.asm in the same PE32+ toolchain as everything else rather
# than requiring a second, incompatible object format.
ASFLAGS := -f win64

$(BUILD_DIR)/main.o: main.c include/efi_types.h include/efi_tables.h include/efi_boot_services.h include/efi_loaded_image_protocol.h include/efi_file_protocol.h include/memory_map.h include/serial.h include/boot_defs.h include/boot_info.h include/kernel_loader.h include/paging.h | $(BUILD_DIR)
	$(CC) $(CFLAGS) main.c -o $(BUILD_DIR)/main.o

$(BUILD_DIR)/serial.o: serial.c include/serial.h include/efi_types.h | $(BUILD_DIR)
	$(CC) $(CFLAGS) serial.c -o $(BUILD_DIR)/serial.o

$(BUILD_DIR)/memory_map.o: memory_map.c include/memory_map.h include/efi_boot_services.h include/boot_defs.h | $(BUILD_DIR)
	$(CC) $(CFLAGS) memory_map.c -o $(BUILD_DIR)/memory_map.o

$(BUILD_DIR)/paging.o: paging.c include/paging.h include/boot_defs.h | $(BUILD_DIR)
	$(CC) $(CFLAGS) paging.c -o $(BUILD_DIR)/paging.o

$(BUILD_DIR)/kernel_loader.o: kernel_loader.c include/kernel_loader.h include/elf.h include/boot_defs.h | $(BUILD_DIR)
	$(CC) $(CFLAGS) kernel_loader.c -o $(BUILD_DIR)/kernel_loader.o

$(BUILD_DIR)/trampoline.o: trampoline.asm | $(BUILD_DIR)
	$(AS) $(ASFLAGS) trampoline.asm -o $(BUILD_DIR)/trampoline.o

$(TARGET): $(BUILD_DIR)/main.o $(BUILD_DIR)/serial.o $(BUILD_DIR)/memory_map.o $(BUILD_DIR)/paging.o $(BUILD_DIR)/kernel_loader.o $(BUILD_DIR)/trampoline.o
	$(LD) $(LDFLAGS) $(BUILD_DIR)/main.o $(BUILD_DIR)/serial.o $(BUILD_DIR)/memory_map.o $(BUILD_DIR)/paging.o $(BUILD_DIR)/kernel_loader.o $(BUILD_DIR)/trampoline.o -o $(TARGET)
	@echo "Built $(TARGET)"

clean:
	rm -rf $(BUILD_DIR)
