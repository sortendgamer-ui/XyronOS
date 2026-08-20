//! gdt.rs — the kernel's own GDT (Global Descriptor Table) and TSS
//! (Task State Segment). See `docs/kernel/INTERRUPTS_DESIGN.md` for
//! the full design reasoning (why a TSS with no ring-3 code yet, why
//! CS reload needs a far-return trick, initialization order).
//!
//! Descriptor bit layout here is dictated by the x86_64 architecture
//! itself (Intel SDM Vol. 3A Section 3.4.5 / 7.2.1; AMD64 APM Vol. 2
//! Section 4.8) — an external hardware specification, implemented
//! independently, the same category as every other CPU/firmware
//! structure this project has built so far (UEFI structs in `boot/`,
//! ELF structures, page-table entries in `mm/page_table_entry.rs`).

use core::mem::size_of;

/// One 8-byte GDT descriptor (code or data segment). Base and limit
/// are architecturally ignored by the CPU for code/data segment
/// access in 64-bit long mode (flat memory model is forced) — this
/// builder still encodes conventional values (base 0, limit
/// `0xFFFFF` with the granularity bit set) purely for structural
/// completeness matching the documented format, not because the CPU
/// checks them here. Only the access byte and the long-mode (`L`)
/// flag bit are actually load-bearing for a code/data descriptor.
const fn segment_descriptor(access: u8, long_mode: bool) -> u64 {
    let limit_low: u64 = 0xFFFF;
    let base: u64 = 0; // ignored for code/data access in long mode
    let flags: u64 = if long_mode { 0b1010 } else { 0b1100 }; // AVL=0,L,D/B,G=1
    let limit_high: u64 = 0xF;

    limit_low
        | (base << 16)
        | ((access as u64) << 40)
        | (limit_high << 48)
        | (flags << 52)
        | (base << 56) // base bits 24:31, still 0
}

// Access byte bits: P(1) DPL(2) S(1) E(1) DC(1) RW(1) A(1) — MSB to LSB.
const ACCESS_KERNEL_CODE: u8 = 0b1001_1010; // P=1 DPL=00 S=1 E=1 DC=0 RW=1 A=0
const ACCESS_KERNEL_DATA: u8 = 0b1001_0010; // P=1 DPL=00 S=1 E=0 DC=0 RW=1 A=0
const ACCESS_TSS: u8 = 0b1000_1001; // P=1 DPL=00 S=0 Type=0b1001 (64-bit TSS, available)

pub const KERNEL_CODE_SELECTOR: u16 = 0x08; // index 1
pub const KERNEL_DATA_SELECTOR: u16 = 0x10; // index 2
const TSS_SELECTOR: u16 = 0x18; // index 3 (occupies indices 3-4)

/// IST index used by the double-fault handler — see
/// `docs/kernel/INTERRUPTS_DESIGN.md` for why double fault
/// specifically needs a dedicated, always-valid stack rather than
/// running on whatever stack was active when it fired. IST indices
/// are 1-based in the architecture's own numbering (0 means "don't
/// switch stacks"), so this is IST slot 1, stored at
/// `interrupt_stack_table[0]` in the TSS below.
pub const DOUBLE_FAULT_IST_INDEX: u16 = 1;

const DOUBLE_FAULT_STACK_SIZE: usize = 4096 * 4; // 16 KiB

/// The double-fault handler's dedicated stack. Never accessed by Rust
/// code at all — only its address is taken and handed to the CPU via
/// the TSS; the CPU itself writes/reads through it as a stack once
/// installed. Zero-initialized in `.bss`, per the architecture's
/// requirement that IST stacks simply be valid, writable memory — no
/// special contents needed.
#[repr(align(16))]
struct DoubleFaultStack([u8; DOUBLE_FAULT_STACK_SIZE]);
static mut DOUBLE_FAULT_STACK: DoubleFaultStack = DoubleFaultStack([0; DOUBLE_FAULT_STACK_SIZE]);

/// The Task State Segment. Field layout and size (104 bytes) are
/// fixed by the architecture (Intel SDM Vol. 3A Section 7.7) — even
/// though this kernel doesn't use TSS's original privilege-level
/// stack-switching purpose (`privilege_stack_table`, left zeroed; no
/// ring-3 code exists yet), the struct's shape is still mandatory
/// because `interrupt_stack_table` (which this subsystem DOES use)
/// lives at a fixed offset within it.
#[repr(C, packed)]
struct TaskStateSegment {
    reserved_1: u32,
    privilege_stack_table: [u64; 3],
    reserved_2: u64,
    interrupt_stack_table: [u64; 7],
    reserved_3: u64,
    reserved_4: u16,
    iomap_base: u16,
}

static mut TSS: TaskStateSegment = TaskStateSegment {
    reserved_1: 0,
    privilege_stack_table: [0; 3],
    reserved_2: 0,
    interrupt_stack_table: [0; 7],
    reserved_3: 0,
    reserved_4: 0,
    // Points past the end of the TSS itself (no I/O permission bitmap
    // is used) — the architecturally-correct way to say "no I/O
    // bitmap present," per the SDM.
    iomap_base: size_of::<TaskStateSegment>() as u16,
};

/// 5 GDT slots: null, kernel code, kernel data, and the 16-byte TSS
/// descriptor (occupying slots 3 and 4 — a system-segment descriptor
/// is twice the width of a code/data descriptor in long mode, since it
/// must hold a full 64-bit base address).
static mut GDT: [u64; 5] = [0; 5];

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

/// Build the TSS descriptor's two 64-bit words from the TSS's actual
/// runtime address — unlike code/data descriptors, a system-segment
/// descriptor's base address is real and load-bearing (it's how the
/// CPU finds the TSS when `ltr` is executed and on every interrupt
/// entry that consults the IST).
fn tss_descriptor(tss_addr: u64) -> (u64, u64) {
    let limit = (size_of::<TaskStateSegment>() - 1) as u64; // 0x67
    let base_low_24 = tss_addr & 0xFF_FFFF;
    let base_mid_8 = (tss_addr >> 24) & 0xFF;
    let base_high_32 = tss_addr >> 32;

    let low = limit | (base_low_24 << 16) | ((ACCESS_TSS as u64) << 40) | (base_mid_8 << 56);
    let high = base_high_32;
    (low, high)
}

/// # Safety
/// Must be called exactly once, early in kernel boot, before the IDT
/// is loaded (`idt::init`) — IDT entries reference
/// `KERNEL_CODE_SELECTOR`, which only becomes a CPU-recognized,
/// loaded selector after this function runs. Must not be called
/// concurrently with anything else touching the `static mut` GDT/TSS
/// storage (true by construction: this kernel is still strictly
/// single-threaded at this point in boot — see
/// `kernel/src/sync/spinlock.rs`'s own note on the same fact).
pub unsafe fn init() {
    let tss_addr = core::ptr::addr_of!(TSS) as u64;
    // SAFETY: DOUBLE_FAULT_STACK is a valid, statically-allocated
    // array; taking the address of one-past-its-end for use as an
    // initial (downward-growing) stack pointer is the standard,
    // correct technique — the stack must start at its HIGH address
    // and grow down, never written to until an actual double fault
    // pushes onto it.
    let double_fault_stack_top =
        (core::ptr::addr_of!(DOUBLE_FAULT_STACK) as u64) + DOUBLE_FAULT_STACK_SIZE as u64;
    TSS.interrupt_stack_table[(DOUBLE_FAULT_IST_INDEX - 1) as usize] = double_fault_stack_top;

    GDT[0] = 0; // null descriptor — architecturally required
    GDT[1] = segment_descriptor(ACCESS_KERNEL_CODE, true);
    GDT[2] = segment_descriptor(ACCESS_KERNEL_DATA, false);
    let (tss_low, tss_high) = tss_descriptor(tss_addr);
    GDT[3] = tss_low;
    GDT[4] = tss_high;

    let gdt_ptr = DescriptorTablePointer {
        limit: (size_of::<[u64; 5]>() - 1) as u16,
        base: core::ptr::addr_of!(GDT) as u64,
    };

    // SAFETY: gdt_ptr describes the GDT array constructed immediately
    // above, valid for the lifetime of this static. LGDT only loads
    // the table's location; it does not itself change any active
    // segment register, which is why the reload below is still
    // required afterward.
    core::arch::asm!("lgdt [{}]", in(reg) &gdt_ptr, options(readonly, nostack, preserves_flags));

    reload_segments();

    // SAFETY: TSS_SELECTOR (GDT index 3) was just populated with a
    // valid TSS descriptor above; LTR requires the GDT to already be
    // loaded (done) and CS to already be a valid code segment (also
    // done, by reload_segments having just run).
    core::arch::asm!("ltr {0:x}", in(reg) TSS_SELECTOR, options(nostack, preserves_flags));
}

/// Reload every segment register to point at the new GDT's selectors.
/// `CS` cannot be written directly by `mov` on any x86 CPU in any
/// mode — the standard long-mode technique is a far return: push the
/// target selector and a target address onto the stack, then `retfq`
/// pops both and jumps, which the CPU treats as a genuine far
/// control transfer (reloading `CS` as part of it) rather than a
/// same-segment jump.
unsafe fn reload_segments() {
    core::arch::asm!(
        "push {sel}",
        "lea {tmp}, [55f + rip]",
        "push {tmp}",
        "retfq",
        "55:",
        sel = in(reg) KERNEL_CODE_SELECTOR as u64,
        tmp = lateout(reg) _,
        options(preserves_flags),
    );

    core::arch::asm!(
        "mov ds, {sel:x}",
        "mov es, {sel:x}",
        "mov ss, {sel:x}",
        "mov fs, {sel:x}",
        "mov gs, {sel:x}",
        sel = in(reg) KERNEL_DATA_SELECTOR,
        options(nostack, preserves_flags),
    );
}
