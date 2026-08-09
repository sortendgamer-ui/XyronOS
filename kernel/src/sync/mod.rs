//! sync/mod.rs — kernel synchronization primitives.
//!
//! Currently just `SpinLock`, introduced because the kernel heap
//! allocator's global statics need `Sync` and this kernel has no
//! OS-provided synchronization primitive to reach for — see
//! `docs/kernel/MEMORY_MANAGER_DESIGN.md`'s "Concrete decisions" for
//! the kernel heap allocator. `sync/` joins `arch/` and `mm/` in the
//! module tree per ADR-006's module layout, which already anticipates
//! new subsystem-driven directories being added this way.

pub mod spinlock;

pub use spinlock::SpinLock;
