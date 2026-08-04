//! mm/mod.rs — memory manager subsystem root.
//!
//! Per ADR-006's module layout and `docs/kernel/MEMORY_MANAGER_DESIGN.md`:
//! this subsystem currently implements the physical frame allocator
//! only. The virtual memory manager and kernel heap allocator the
//! design document also describes are NOT implemented here — per the
//! "do not proceed to the next kernel subsystem" rule, they are a
//! later addition to this same module, not scaffolded in advance.

pub mod frame_allocator;
pub mod memory_map;
pub mod phys_addr;

#[cfg_attr(test, allow(unused_imports))]
pub use frame_allocator::FrameAllocator;
#[cfg_attr(test, allow(unused_imports))]
pub use phys_addr::PhysAddr;
