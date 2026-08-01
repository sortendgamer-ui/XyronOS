/*
 * serial.h — Minimal 16550-compatible UART driver, COM1 only.
 *
 * See ADR-004 for why this exists: it is the bootloader's only output
 * channel after ExitBootServices succeeds, since ConOut is no longer
 * guaranteed usable at that point. This is intentionally narrow in
 * scope — output only, no input, no interrupts, one port (COM1) — a
 * full multi-port interrupt-driven UART driver is Phase 4 work for the
 * kernel proper, not the bootloader.
 */

#ifndef OS_SERIAL_H
#define OS_SERIAL_H

#include "efi_types.h"

/* Standard IBM PC COM1 base I/O port. COM2/3/4 exist at other
 * well-known ports but are out of scope — this driver exists for
 * bootloader diagnostics on the port QEMU's -serial stdio and most
 * real hardware's debug header expose by default. */
#define SERIAL_COM1_BASE 0x3F8

/*
 * SerialInit — program the UART for 38400 baud, 8 data bits, no
 * parity, 1 stop bit (8N1), with FIFOs enabled.
 *
 * Explicitly sets the baud rate divisor rather than assuming firmware
 * already configured the port usably — see ADR-004: we cannot rely on
 * any firmware-provided state once we are past ExitBootServices, and
 * on real hardware nothing guarantees the port was configured at all
 * before we got here.
 *
 * Must be called before any Serial* write function.
 */
void SerialInit(void);

/*
 * SerialWriteChar — write a single byte out COM1, blocking until the
 * UART's transmit holding register is empty. Translates '\n' to
 * '\r\n' so output lines start at column 0 on a real terminal, the
 * same convention used for ConOut strings elsewhere in this
 * bootloader.
 */
void SerialWriteChar(char c);

/*
 * SerialWriteString — write a null-terminated CHAR8 (narrow ASCII)
 * string. Bootloader diagnostic text is always ASCII, so this takes
 * CHAR8*, not CHAR16* — no UCS-2 conversion needed here, unlike the
 * ConOut path in main.c's PrintAscii.
 */
void SerialWriteString(const CHAR8 *str);

/*
 * SerialWriteHex64 — write a UINT64 as a fixed-width 16-digit
 * hexadecimal string (e.g. "00000000DEADBEEF"). Used to report memory
 * map addresses and sizes, which are meaningless in decimal at these
 * magnitudes and error-prone to format correctly without a working
 * libc — we are freestanding, so there is no printf to reach for.
 */
void SerialWriteHex64(UINT64 value);

#endif /* OS_SERIAL_H */
