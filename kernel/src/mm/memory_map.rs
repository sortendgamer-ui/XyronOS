//! memory_map.rs — iterates the raw UEFI memory map handed off in
//! `BootInfo`.
//!
//! Reads each descriptor's fields at FIXED BYTE OFFSETS
//! (`Type` at 0, `PhysicalStart` at 8, `NumberOfPages` at 24) rather
//! than casting the buffer to a Rust struct array. This is the same
//! lesson `boot/main.c` already applied when walking this same memory
//! map on the bootloader side: the UEFI spec allows firmware to report
//! a `DescriptorSize` larger than any fixed struct definition (to
//! reserve room for future spec fields), but guarantees the position
//! of the fields that already exist never changes. Reading by fixed
//! offset with `descriptor_size` as the iteration stride is correct
//! regardless of which case we're in; casting to
//! `[SomeStruct; entry_count]` would silently misread every entry
//! after the first if the real stride is larger than `SomeStruct`'s
//! size — exactly what was observed in practice
//! (`boot/main.c` found a 48-byte stride against a naive 40-byte
//! struct layout).

use crate::boot_info::BootInfo;
use crate::mm::phys_addr::PhysAddr;

/// UEFI memory type value for memory that is immediately usable —
/// per UEFI Spec Table 7-9, `EfiConventionalMemory` is type value 7.
/// This is the ONLY type this allocator ever treats as free; every
/// other value (including ones this constant list doesn't name)
/// stays marked used, which is always the safe direction to be wrong
/// in if a firmware ever reports a type this code doesn't recognize.
pub const EFI_CONVENTIONAL_MEMORY: u32 = 7;

/// One parsed memory map entry — only the three fields this subsystem
/// actually uses (`VirtualStart` and `Attribute` are read by nothing
/// here, per the no-placeholder-fields rule; they can be added if a
/// future subsystem needs them).
#[derive(Debug, Clone, Copy)]
pub struct MemoryMapEntry {
    pub typ: u32,
    pub physical_start: PhysAddr,
    pub number_of_pages: u64,
}

impl MemoryMapEntry {
    /// End address (exclusive) of this region, or `None` if
    /// `physical_start + number_of_pages * FRAME_SIZE` would overflow
    /// a `u64` — a malformed entry a caller must reject rather than
    /// silently wrap.
    pub fn end_addr(&self) -> Option<u64> {
        let size = self
            .number_of_pages
            .checked_mul(crate::mm::phys_addr::FRAME_SIZE)?;
        self.physical_start.as_u64().checked_add(size)
    }
}

pub struct MemoryMapIter<'a> {
    base_ptr: *const u8,
    descriptor_size: usize,
    entry_count: usize,
    index: usize,
    _lifetime: core::marker::PhantomData<&'a ()>,
}

impl<'a> MemoryMapIter<'a> {
    /// Construct an iterator over `boot_info`'s memory map.
    ///
    /// # Safety
    /// `boot_info.memory_map_phys_addr` must point to at least
    /// `boot_info.memory_map_size_bytes` valid, readable bytes for the
    /// duration this iterator is used — true for the real `BootInfo`
    /// the bootloader populates (the buffer falls inside the
    /// identity-mapped region and is never freed, per ADR-005), which
    /// is the only kind of `BootInfo` this kernel ever receives.
    pub unsafe fn new(boot_info: &'a BootInfo) -> Self {
        MemoryMapIter {
            base_ptr: boot_info.memory_map_phys_addr as *const u8,
            descriptor_size: boot_info.memory_map_descriptor_size as usize,
            entry_count: boot_info.memory_map_entry_count as usize,
            index: 0,
            _lifetime: core::marker::PhantomData,
        }
    }
}

impl<'a> Iterator for MemoryMapIter<'a> {
    type Item = MemoryMapEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.entry_count {
            return None;
        }

        let offset = self.index * self.descriptor_size;
        self.index += 1;

        // SAFETY: the caller of `MemoryMapIter::new` guaranteed
        // `base_ptr` is valid for `entry_count * descriptor_size`
        // bytes; `FrameAllocator::init` additionally re-validates that
        // `memory_map_size_bytes >= entry_count * descriptor_size`
        // before ever constructing this iterator, so `offset + 32`
        // (the last byte this function reads) is within bounds for
        // every entry this loop visits. `read_unaligned` is required
        // because `descriptor_size` is firmware-chosen and gives no
        // alignment guarantee for these reads.
        unsafe {
            let entry_ptr = self.base_ptr.add(offset);
            let typ = core::ptr::read_unaligned(entry_ptr as *const u32);
            let physical_start = core::ptr::read_unaligned(entry_ptr.add(8) as *const u64);
            let number_of_pages = core::ptr::read_unaligned(entry_ptr.add(24) as *const u64);

            Some(MemoryMapEntry {
                typ,
                physical_start: PhysAddr::new(physical_start),
                number_of_pages,
            })
        }
    }
}
