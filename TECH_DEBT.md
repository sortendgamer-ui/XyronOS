# Technical Debt

Known limitations that are correct (nothing here is a bug — see
CHANGELOG.md's "Fixed" sections for actual bugs found and fixed) but
not optimal, recorded so they aren't rediscovered from scratch later
and aren't silently forgotten. An item here is not scheduled work —
see [TODO.md](TODO.md) for feature requests and [ROADMAP.md](ROADMAP.md)
for what's actually planned.

## Bootloader (Phase 2, frozen)

### Page-table pages the bootloader itself builds are not address-capped
**Where:** `boot/paging.c`, `AllocateZeroedPage` (used for the PML4,
identity-map PDPT/PDs, and the higher-half kernel PDPT/PD).

ADR-005's Part 4 work capped every allocation the kernel would need to
directly dereference after the jump (kernel image, memory map buffer,
`BootInfo`, kernel stack) below `IDENTITY_MAP_LIMIT` (4 GiB) via
`AllocateMaxAddress`. The page-table pages `paging.c` builds for
itself were not included in that capping — they use unconstrained
`AllocatePages(AllocateAnyPages, ...)`.

This matters because the kernel's virtual memory manager
(`kernel/src/mm/vmm.rs`, Phase 3) needs to walk and modify these same
tables (reading CR3, following PML4→PDPT→PD→PT pointers) — which
requires dereferencing their physical addresses through the identity
map, exactly like any other kernel-visible allocation. If any of these
pages were ever allocated above 4 GiB by firmware, the VMM would have
no way to reach them.

**Status: never observed to actually happen.** Every boot test run
against this project (QEMU/OVMF, the only platform tested so far) has
placed every page-table page well under 4 GiB. Per this project's own
rule — the bootloader is frozen and not modified without an actual,
observed bug — this is NOT being fixed in `boot/paging.c`
speculatively. Instead, the kernel's VMM defensively checks every
page-table physical address it encounters against
`IDENTITY_MAP_LIMIT` before dereferencing it, and returns a clear
error (rather than silently misbehaving or corrupting memory) if the
assumption is ever violated. If that error is ever actually observed
on real hardware, THAT would be the documented bug justifying a
`boot/paging.c` change (capping its allocations the same way Part 4's
other allocations already are) — recorded here so the fix is obvious
if that day comes.

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

## Kernel Heap Allocator (Phase 3)

### Leftover alignment/sizing gaps smaller than one block header are permanently lost
**Where:** `kernel/src/mm/linked_list_allocator.rs`, `LinkedListAllocator::alloc`.

When a free block is larger than a request (or its start isn't already
aligned to what the request needs), the leftover space on either side
is only recoverable as a new free block if it's large enough to hold
its own `FreeListNode` header. Smaller leftovers become permanent
internal fragmentation for the lifetime of the heap. This is a
well-known, accepted property of simple linked-list allocators in
general (not a defect specific to this implementation) — see
`docs/kernel/MEMORY_MANAGER_DESIGN.md`'s "Concrete decisions" for the
full reasoning. Not fixed, because fixing it (e.g. a slab/size-class
allocator layered on top for small, fixed-size requests) is a real
allocator redesign, explicitly deferred to Phase 18 (Optimization)
alongside the frame allocator's own O(n)-scan and bitmap-sizing
deferrals.

### Heap growth never shrinks back
**Where:** `kernel/src/mm/heap.rs`, `KernelHeap::grow`.

Once a page is mapped into the heap region, it stays mapped for the
kernel's lifetime — even if every allocation within it is later freed,
that page is never unmapped and its frame is never returned to the
physical frame allocator. For a kernel with no long-running "high
watermark then quiet" workload yet (nothing generates that pattern
until much later phases), this has no observed impact, but it is a
real, permanent limitation as written. A shrink pass (unmap pages
whose entire range the free list reports as unused) is a legitimate
future improvement, not implemented speculatively now.

### `SpinLock` has never been contention-tested
**Where:** `kernel/src/sync/spinlock.rs`.

This kernel runs strictly single-threaded through every subsystem
built so far (no interrupts enabled, no second CPU brought up, no
scheduler). `SpinLock`'s unit tests confirm it provides correct
interior mutability and that a guard's `Drop` releases the lock, but
none of them — and nothing in the boot self-tests — exercises actual
concurrent contention (two execution contexts genuinely racing for the
same lock at the same time), since no such contexts exist yet in this
kernel. The implementation is a standard, textbook-correct atomic
spinlock, not a stand-in — but "correct under real contention" remains
unverified until interrupts and/or the scheduler exist to create any.
