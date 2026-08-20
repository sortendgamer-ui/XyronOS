//! idt.rs — the IDT (Interrupt Descriptor Table) data structure and
//! loading mechanics. Handler *implementations* live in
//! `exceptions.rs`; this file owns only the table itself and the
//! entry format — see `docs/kernel/INTERRUPTS_DESIGN.md` for why
//! these are kept separate.
//!
//! Entry bit layout (16 bytes per entry in long mode: offset split
//! across three fields, selector, IST index, type/attribute byte) is
//! dictated by the x86_64 architecture (Intel SDM Vol. 3A Section
//! 6.14.1) — an external hardware specification, implemented
//! independently, same category as `gdt.rs`.

use crate::arch::x86_64::gdt::KERNEL_CODE_SELECTOR;
use core::mem::size_of;

/// Interrupt-gate type/attribute byte: P=1, DPL=00, must-be-0-bit,
/// Type=0xE (32/64-bit interrupt gate). An interrupt gate (as opposed
/// to a trap gate, Type=0xF) additionally clears `EFLAGS.IF` on entry
/// — irrelevant to whether a CPU exception is delivered (exceptions
/// aren't gated by `IF` either way, per the design doc), but interrupt
/// gates are the conventional, more conservative choice for exception
/// handlers and cost nothing here since this subsystem never expects
/// a maskable hardware interrupt to legitimately fire during one of
/// these handlers anyway.
const TYPE_ATTR_INTERRUPT_GATE: u8 = 0x8E;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(super) struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const fn missing() -> Self {
        IdtEntry {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0, // Present bit clear — "entry does not exist"
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    /// Point this entry at `handler`, using `KERNEL_CODE_SELECTOR`
    /// (the only code selector this kernel has — see `gdt.rs`) and
    /// optionally an IST index (0 = don't switch stacks, per the
    /// architecture's own numbering — see `gdt.rs`'s
    /// `DOUBLE_FAULT_IST_INDEX` for the one nonzero case this kernel
    /// uses).
    fn set_handler(&mut self, handler: u64, ist: u8) {
        self.offset_low = handler as u16;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.selector = KERNEL_CODE_SELECTOR;
        self.ist = ist;
        self.type_attr = TYPE_ATTR_INTERRUPT_GATE;
    }
}

/// All 256 possible vectors exist in storage, but only 0-31 (the
/// architecturally-defined exceptions) are ever installed with a real
/// handler by `init` — see the design doc's "IDT design" section for
/// why leaving 32-255 present-bit-clear (via `IdtEntry::missing`,
/// which this array starts fully initialized to) is deliberate, not
/// an oversight: an interrupt arriving there before a later subsystem
/// installs a real handler produces a well-defined CPU fault instead
/// of jumping to nothing.
static mut IDT: [IdtEntry; 256] = [IdtEntry::missing(); 256];

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

/// # Safety
/// Must be called exactly once, after `gdt::init()` has already run
/// (IDT entries reference `KERNEL_CODE_SELECTOR`, which only becomes
/// a valid, loaded selector once the GDT is active — see
/// `docs/kernel/INTERRUPTS_DESIGN.md`'s initialization order). Must
/// not be called concurrently with anything else touching the
/// `static mut` IDT storage (true by construction — see `gdt::init`'s
/// identical note on single-threadedness at this point in boot).
pub unsafe fn init() {
    crate::arch::x86_64::exceptions::install_handlers(&mut IDT);

    let idt_ptr = DescriptorTablePointer {
        limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: core::ptr::addr_of!(IDT) as u64,
    };

    // SAFETY: idt_ptr describes the IDT array populated immediately
    // above (via install_handlers), valid for the lifetime of this
    // static.
    core::arch::asm!("lidt [{}]", in(reg) &idt_ptr, options(readonly, nostack, preserves_flags));
}

/// Called only from `exceptions.rs`'s `install_handlers` — kept as a
/// crate-visible function here (rather than making `IdtEntry` and its
/// `set_handler` method `pub` more broadly) so the IDT's raw storage
/// stays owned and encapsulated by this file even though the handler
/// *addresses* are decided in `exceptions.rs`.
pub(super) fn set_entry(idt: &mut [IdtEntry; 256], vector: usize, handler: u64, ist: u8) {
    idt[vector].set_handler(handler, ist);
}
