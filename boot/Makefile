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

$(BUILD_DIR)/main.o: main.c include/efi_types.h include/efi_tables.h | $(BUILD_DIR)
	$(CC) $(CFLAGS) main.c -o $(BUILD_DIR)/main.o

$(TARGET): $(BUILD_DIR)/main.o
	$(LD) $(LDFLAGS) $(BUILD_DIR)/main.o -o $(TARGET)
	@echo "Built $(TARGET)"

clean:
	rm -rf $(BUILD_DIR)
