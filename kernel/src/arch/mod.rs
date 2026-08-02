//! arch/mod.rs — architecture-specific code, gated by target so a
//! future non-x86_64 port (not currently planned — ADR-001/ADR-006
//! scope this project to x86_64 only for now) would add a sibling
//! module here rather than requiring changes throughout the kernel.

pub mod x86_64;
