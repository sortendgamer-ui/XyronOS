//! serial.rs — minimal 16550-compatible UART driver, COM1 only, for
//! early kernel boot diagnostics.
//!
//! Reimplemented independently from `boot/serial.c` rather than
//! shared — ADR-004 already established that the bootloader's serial
//! handling is not linked into the kernel; the kernel owns its own
//! device state from the moment it starts (the same principle
//! `tests/kernel_stub/kernel_stub.c` demonstrated by re-initializing
//! the UART even though the bootloader had just done so). This is
//! explicitly NOT the formal driver model Phase 4 will define — see
//! ADR-006's "Early debug output vs. Phase 4 drivers".
//!
//! Register layout and initialization sequence are the same
//! documented 16550 hardware behavior `boot/serial.c` used — an
//! external hardware specification, implemented independently in a
//! second language, not code shared or copied between the two.

const COM1: u16 = 0x3F8;

const REG_DATA: u16 = 0;
const REG_INT_ENABLE: u16 = 1;
const REG_FIFO_CTRL: u16 = 2;
const REG_LINE_CTRL: u16 = 3;
const REG_MODEM_CTRL: u16 = 4;
const REG_LINE_STATUS: u16 = 5;

const LCR_8N1: u8 = 0x03;
const LCR_DLAB: u8 = 0x80;

const FCR_ENABLE_FIFO: u8 = 0x01;
const FCR_CLEAR_RX_FIFO: u8 = 0x02;
const FCR_CLEAR_TX_FIFO: u8 = 0x04;
const FCR_14BYTE_TRIGGER: u8 = 0xC0;

const MCR_DTR: u8 = 0x01;
const MCR_RTS: u8 = 0x02;
const MCR_OUT2: u8 = 0x08;

const LSR_TX_EMPTY: u8 = 0x20;

/// 1.8432 MHz UART clock / (38400 baud * 16) = 3 — the standard 16550
/// divisor for 38400 baud, matching `boot/serial.c`'s choice so a
/// terminal watching the boot process doesn't need to change baud
/// rate mid-stream when control passes from bootloader to kernel.
const BAUD_DIVISOR_38400: u16 = 3;

/// SAFETY: `out`/`in` on a fixed, always-valid I/O port number is sound
/// as a primitive operation — the actual safety burden (not racing
/// with another core, not doing this before paging/interrupts are in
/// a state that allows it) is on the caller, which is why these stay
/// private to this module and only `init`/`write_*` are exposed.
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
    value
}

/// Program the UART for 38400 8N1 with FIFOs enabled. Must be called
/// before any `write_*` function — matches `SerialInit()` in
/// `boot/serial.c` field-for-field; see that file's comments for why
/// each step is necessary rather than assumed.
pub fn init() {
    // SAFETY: COM1's I/O ports are fixed, architecturally-defined
    // addresses; this sequence is the standard documented 16550
    // initialization procedure, called once at kernel boot before any
    // interrupt handling exists to race with it.
    unsafe {
        outb(COM1 + REG_INT_ENABLE, 0x00);

        outb(COM1 + REG_LINE_CTRL, LCR_DLAB);
        outb(COM1 + REG_DATA, (BAUD_DIVISOR_38400 & 0xFF) as u8);
        outb(COM1 + REG_INT_ENABLE, ((BAUD_DIVISOR_38400 >> 8) & 0xFF) as u8);
        outb(COM1 + REG_LINE_CTRL, LCR_8N1);

        outb(
            COM1 + REG_FIFO_CTRL,
            FCR_ENABLE_FIFO | FCR_CLEAR_RX_FIFO | FCR_CLEAR_TX_FIFO | FCR_14BYTE_TRIGGER,
        );

        outb(COM1 + REG_MODEM_CTRL, MCR_DTR | MCR_RTS | MCR_OUT2);
    }
}

fn write_byte(byte: u8) {
    if byte == b'\n' {
        write_byte(b'\r');
    }

    // SAFETY: polling the Line Status Register before writing is the
    // documented-correct way to avoid overwriting a byte the UART
    // hasn't shifted out yet; no interrupt-driven alternative exists
    // this early in kernel boot (no IDT installed yet).
    unsafe {
        while (inb(COM1 + REG_LINE_STATUS) & LSR_TX_EMPTY) == 0 {}
        outb(COM1 + REG_DATA, byte);
    }
}

/// Write a UTF-8 string. Only the ASCII subset is meaningful over a
/// raw serial terminal in this early-boot context, but taking `&str`
/// (not `&[u8]`) keeps call sites using ordinary Rust string literals
/// rather than byte-string literals throughout the kernel.
pub fn write_str(s: &str) {
    for byte in s.bytes() {
        write_byte(byte);
    }
}

/// Write a `u64` as a fixed-width 16-digit hex string — mirrors
/// `SerialWriteHex64` in `boot/serial.c`, for the same reason
/// (addresses and sizes at these magnitudes are meaningless in
/// decimal, and there is no `println!`/formatting machinery this
/// early — `core::fmt` integration is a natural follow-up once the
/// kernel heap exists, not implemented speculatively now).
pub fn write_hex64(value: u64) {
    const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    let mut buf = [0u8; 16];
    let mut v = value;
    for slot in buf.iter_mut().rev() {
        *slot = HEX_DIGITS[(v & 0xF) as usize];
        v >>= 4;
    }
    for &b in &buf {
        write_byte(b);
    }
}
