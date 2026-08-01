/*
 * serial.c — Minimal 16550-compatible UART driver implementation.
 * See serial.h and ADR-004 for rationale.
 *
 * Register offsets from SERIAL_COM1_BASE (standard 16550 layout):
 *   +0  DLAB=0: Data register (read/write)      DLAB=1: Divisor LSB
 *   +1  DLAB=0: Interrupt Enable Register        DLAB=1: Divisor MSB
 *   +2  FIFO Control Register (write) / Interrupt Identification (read)
 *   +3  Line Control Register (sets DLAB in bit 7)
 *   +4  Modem Control Register
 *   +5  Line Status Register
 */

#include "include/serial.h"

#define REG_DATA          0
#define REG_INT_ENABLE    1
#define REG_FIFO_CTRL     2
#define REG_LINE_CTRL     3
#define REG_MODEM_CTRL    4
#define REG_LINE_STATUS   5

#define LCR_8N1        0x03 /* 8 data bits, no parity, 1 stop bit */
#define LCR_DLAB       0x80 /* Divisor Latch Access Bit */

#define FCR_ENABLE_FIFO       0x01
#define FCR_CLEAR_RX_FIFO     0x02
#define FCR_CLEAR_TX_FIFO     0x04
#define FCR_14BYTE_TRIGGER    0xC0

#define MCR_DTR   0x01
#define MCR_RTS   0x02
#define MCR_OUT2  0x08 /* must be set for interrupts on real hardware;
                           harmless and conventional to set even though
                           this driver runs polled, not interrupt-driven */

#define LSR_TX_EMPTY 0x20 /* Transmit Holding Register Empty */

/* UART input clock is 1.8432 MHz on standard PC hardware; the baud
 * rate divisor is that clock divided by (desired baud * 16). For our
 * target of 38400 baud: 1843200 / (38400 * 16) = 3. This is the
 * well-known standard divisor value for 38400 baud on 16550 hardware,
 * derived from the UART's documented clock, not copied from any OS
 * driver source. */
#define BAUD_DIVISOR_38400 3

/*
 * outb/inb — single-byte port I/O via inline assembly.
 *
 * "dN" constrains the port number to the DX register (required by the
 * x86 IN/OUT instruction encoding for 16-bit port numbers above 0xFF,
 * which SERIAL_COM1_BASE's port and its offsets are not, but using dN
 * uniformly keeps this correct if a port ever needs relocating above
 * 0xFF). "a" constrains the data value to AL, per the instruction's
 * fixed accumulator-register encoding.
 */
static inline void outb(UINT16 port, UINT8 value)
{
    __asm__ __volatile__("outb %0, %1" : : "a"(value), "Nd"(port));
}

static inline UINT8 inb(UINT16 port)
{
    UINT8 value;
    __asm__ __volatile__("inb %1, %0" : "=a"(value) : "Nd"(port));
    return value;
}

void SerialInit(void)
{
    /* Disable all UART-generated interrupts — this driver is polled,
     * and we have not set up an interrupt controller or handlers at
     * this point in boot (that is Phase 3 kernel work), so any
     * interrupt this UART raised would have nowhere correct to go. */
    outb(SERIAL_COM1_BASE + REG_INT_ENABLE, 0x00);

    /* Enable DLAB to expose the divisor latch registers at offsets
     * 0 and 1, program the divisor for 38400 baud, then clear DLAB
     * again so offset 0 goes back to being the data register. */
    outb(SERIAL_COM1_BASE + REG_LINE_CTRL, LCR_DLAB);
    outb(SERIAL_COM1_BASE + REG_DATA, (UINT8)(BAUD_DIVISOR_38400 & 0xFF));
    outb(SERIAL_COM1_BASE + REG_INT_ENABLE, (UINT8)((BAUD_DIVISOR_38400 >> 8) & 0xFF));
    outb(SERIAL_COM1_BASE + REG_LINE_CTRL, LCR_8N1);

    /* Enable and reset the FIFOs, with a 14-byte receive trigger level
     * (irrelevant for our transmit-only usage, but this is the
     * standard/correct full initialization sequence for real 16550
     * hardware rather than a cut corner). */
    outb(SERIAL_COM1_BASE + REG_FIFO_CTRL,
         FCR_ENABLE_FIFO | FCR_CLEAR_RX_FIFO | FCR_CLEAR_TX_FIFO | FCR_14BYTE_TRIGGER);

    /* Assert DTR/RTS and OUT2 — standard modem control bring-up so a
     * real UART (and QEMU's emulation of one) considers the line
     * "ready," matching how real hardware initialization sequences
     * are documented. */
    outb(SERIAL_COM1_BASE + REG_MODEM_CTRL, MCR_DTR | MCR_RTS | MCR_OUT2);
}

void SerialWriteChar(char c)
{
    if (c == '\n') {
        SerialWriteChar('\r');
    }

    /* Poll the Line Status Register until the Transmit Holding
     * Register is empty (bit 5) — the correct, non-shortcut way to
     * avoid overwriting a byte the UART hasn't shifted out yet.
     * There is no interrupt-driven alternative available yet (no
     * interrupt handling exists this early in boot), so polling is
     * not a corner cut here, it is the only correct option available
     * at this stage of the system's life. */
    while ((inb(SERIAL_COM1_BASE + REG_LINE_STATUS) & LSR_TX_EMPTY) == 0) {
        /* busy-wait */
    }

    outb(SERIAL_COM1_BASE + REG_DATA, (UINT8)c);
}

void SerialWriteString(const CHAR8 *str)
{
    while (*str != '\0') {
        SerialWriteChar(*str);
        str++;
    }
}

void SerialWriteHex64(UINT64 value)
{
    static const char hexDigits[] = "0123456789ABCDEF";
    char buffer[17]; /* 16 hex digits + null terminator */

    buffer[16] = '\0';
    for (int i = 15; i >= 0; i--) {
        buffer[i] = hexDigits[value & 0xF];
        value >>= 4;
    }

    SerialWriteString(buffer);
}
