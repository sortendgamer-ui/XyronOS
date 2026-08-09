//! mm/mod.rs — memory manager subsystem root.
//!
//! Per ADR-006's module layout and `docs/kernel/MEMORY_MANAGER_DESIGN.md`:
//! this subsystem now implements the physical frame allocator, the
//! virtual memory manager, and the kernel heap allocator — the full
//! Memory Manager subsystem ADR-006 scoped out is complete as of this
//! module.

pub mod frame_allocator;
pub mod heap;
pub mod linked_list_allocator;
pub mod memory_map;
pub mod page_table_entry;
pub mod phys_addr;
pub mod virt_addr;
pub mod vmm;

#[cfg_attr(test, allow(unused_imports))]
pub use frame_allocator::FrameAllocator;
#[cfg_attr(test, allow(unused_imports))]
pub use page_table_entry::PageFlags;
#[cfg_attr(test, allow(unused_imports))]
pub use phys_addr::PhysAddr;
#[cfg_attr(test, allow(unused_imports))]
pub use virt_addr::VirtAddr;
#[cfg_attr(test, allow(unused_imports))]
pub use vmm::VirtualMemoryManager;
