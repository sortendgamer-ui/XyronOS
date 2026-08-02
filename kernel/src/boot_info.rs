//! boot_info.rs — the Rust half of the ADR-005 BootInfo ABI contract.
//!
//! This struct's field order, types, and therefore memory layout MUST
//! exactly match `boot/include/boot_info.h`'s `BOOT_INFO` C struct —
//! there is no compiler shared between the bootloader (mingw-w64) and
//! this kernel (rustc) to catch a mismatch, so `#[repr(C)]` plus
//! manual field-by-field correspondence is the entire guarantee. If
//! `boot/include/boot_info.h` ever changes, this file changes with it
//! in the same commit, and `BOOTINFO_VERSION` is bumped on both sides
//! per ADR-005.

/// ASCII "XOSBOOT1" as a little-endian u64 — must match
/// `BOOTINFO_MAGIC` in boot/include/boot_info.h exactly (same
/// byte-by-byte construction, so the value's meaning is legible here
/// too, not just copied as an opaque constant).
pub const BOOTINFO_MAGIC: u64 = (b'X' as u64)
    | ((b'O' as u64) << 8)
    | ((b'S' as u64) << 16)
    | ((b'B' as u64) << 24)
    | ((b'O' as u64) << 32)
    | ((b'O' as u64) << 40)
    | ((b'T' as u64) << 48)
    | ((b'1' as u64) << 56);

/// Must match `BOOTINFO_VERSION` in boot/include/boot_info.h.
pub const BOOTINFO_VERSION: u32 = 1;

/// Mirrors `BOOT_INFO` in boot/include/boot_info.h field-for-field, in
/// the same order. `#[repr(C)]` makes rustc apply the same layout and
/// padding rules the C compiler applies for repr(C)/plain structs —
/// natural alignment for each field, which is what boot_info.h relies
/// on implicitly (it has no `#pragma pack` or explicit padding).
#[repr(C)]
pub struct BootInfo {
    pub magic: u64,
    pub version: u32,
    pub struct_size_bytes: u32,

    pub memory_map_phys_addr: u64,
    pub memory_map_size_bytes: u64,
    pub memory_map_descriptor_size: u64,
    pub memory_map_descriptor_version: u32,
    pub memory_map_entry_count: u64,

    pub kernel_physical_base: u64,
    pub kernel_virtual_base: u64,
    pub kernel_size_bytes: u64,

    pub kernel_stack_top: u64,
    pub kernel_stack_size_bytes: u64,
}

impl BootInfo {
    /// Validate the handoff before any other field is trusted — same
    /// three checks `tests/kernel_stub/kernel_stub.c` already proved
    /// work correctly from the bootloader side (magic, version, and
    /// this struct's own recorded size all agreeing).
    pub fn is_valid(&self) -> bool {
        self.magic == BOOTINFO_MAGIC
            && self.version == BOOTINFO_VERSION
            && self.struct_size_bytes as usize == core::mem::size_of::<BootInfo>()
    }
}
