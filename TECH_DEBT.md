# Technical Debt

Known limitations that are correct (nothing here is a bug — see
CHANGELOG.md's "Fixed" sections for actual bugs found and fixed) but
not optimal, recorded so they aren't rediscovered from scratch later
and aren't silently forgotten. An item here is not scheduled work —
see [TODO.md](TODO.md) for feature requests and [ROADMAP.md](ROADMAP.md)
for what's actually planned.

## Memory Manager (Phase 3)

### Frame allocator bitmap is sized off the entire memory map, including MMIO
**Where:** `kernel/src/mm/frame_allocator.rs`, `FrameAllocator::init`.

`total_frames` (and therefore the bitmap's size) is computed from the
highest end address across *every* memory map entry — including
`EfiMemoryMappedIO` regions, which on real hardware and QEMU's q35
machine can report very high physical addresses (a 64-bit PCIe MMIO
window). Observed in practice: a 256 MiB QEMU VM produced a memory map
implying roughly 1 TiB of "tracked" address space, because of one such
high MMIO region — yielding a ~32 MiB bitmap to track a machine with a
few hundred MiB of actual RAM.

This is not incorrect: `MAX_BITMAP_BYTES` (64 MiB) caps how bad this
can get, and every allocate/deallocate/self-test in production and in
CI has passed against this real data. But it wastes both the memory
the bitmap itself occupies and the frame-scan time for `allocate()`'s
worst case.

**Possible fix, not implemented:** compute `total_frames` from the
highest `EfiConventionalMemory` (or otherwise-trackable) region only,
and treat any address above that as simply unmanaged rather than
"tracked but always marked used." Deferred rather than fixed now
because the current behavior is correct and the design document
already scoped allocator performance work to Phase 18.

## Toolchain (Phase 3)

### This development sandbox cannot use `rustup`
Documented in full in `kernel/README.md` rather than duplicated here,
since it's build-environment setup, not a code limitation — recorded
here only as a pointer: apt-packaged `rustc`/`rust-src` required
`RUSTC_BOOTSTRAP=1`, a manually-generated `Cargo.lock` for the
`rust-src` workspace, and a working `ld.lld` in place of a broken
`rust-lld` symlink. CI uses the standard `rustup` path and is
unaffected.

### `cargo test` requires manually specifying `--target <host-triple>`
`kernel/.cargo/config.toml` defaults `cargo build` to the freestanding
custom target, which has no test harness support (no OS, no
`std::rt`). Running the host-target unit tests therefore requires an
explicit `--target` override on every `cargo test` invocation — not
onerous, but a rough edge a `cargo test` alias or a documented shell
function could smooth over. Not done, since it would be pure
convenience tooling with no correctness benefit.

### Two config.toml settings had to be scoped away from `[build]`/`[unstable]`
Both `build-std` (as an `[unstable]` table entry) and `rustflags`
(when placed under `[build]`) apply to *every* cargo invocation
regardless of `--target` — including host-target `cargo test` runs.
The first caused duplicate `core` lang-item link errors; the second
silently linked the kernel's `-Tlinker.ld` (entry point
`0xFFFFFFFF80000000`) into the host test binary, which then segfaulted
on startup with no output. Both are now scoped correctly (`build-std`
passed explicitly per-build; `rustflags` scoped under
`[target.x86_64-os]`) — recorded here as a trap worth knowing about if
`.cargo/config.toml` is ever restructured again.
