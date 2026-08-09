# Building and Testing the Kernel

## Standard path (rustup available)
```bash
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
cd kernel
cargo +nightly build --release -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem
```
`.cargo/config.toml` points `cargo` at `../toolchain/x86_64-os.json` by
default, so no `--target` flag is needed. `-Z build-std` IS required
explicitly on the command line, deliberately NOT baked into
`.cargo/config.toml`'s `[unstable]` table — see "Why build-std and
rustflags are passed explicitly, not in config.toml" below for why.

## Running the unit tests
Unit tests (`mm/*.rs`'s `#[cfg(test)]` modules) run on the HOST
target, not the freestanding kernel target — there is no OS under the
freestanding target for a test harness to run on top of. They test
pure logic (bitmap allocate/free bookkeeping, address arithmetic) in
isolation; the boot-time integration self-test in `main.rs` covers
what these structurally cannot (real UEFI memory map parsing against
this specific boot's real data — see "Boot-testing the kernel" below).

```bash
cd kernel
cargo +nightly test --target x86_64-unknown-linux-gnu --bin neoastrenos-kernel
# (substitute your actual host triple if different — `rustc -vV | grep host`)
```
No `-Z build-std` flag here: the host target already has a prebuilt
`core`/`std`, and passing build-std for it is exactly what causes the
duplicate-lang-item problem described below.

## Why build-std and rustflags are passed explicitly, not in config.toml
Two settings that look natural to put in `.cargo/config.toml` turned
out to break `cargo test` when placed there — both discovered by
actually running the tests, not by inspection, and both now documented
in [`TECH_DEBT.md`](../TECH_DEBT.md):

1. **`build-std` as an `[unstable]` table entry applies globally**,
   regardless of `--target`. A host-target test run would then build
   `core` from source (build-std) AND link the host's prebuilt `core`
   (via prebuilt `std`) into the same binary — two different `core`
   crates, producing `error[E0152]: duplicate lang item`. Fix: pass
   `-Z build-std=...` explicitly only for real (freestanding-target)
   builds, never for `cargo test`.
2. **`rustflags` under `[build]` also applies globally.** The kernel's
   `-Tlinker.ld` (entry point `0xFFFFFFFF80000000`, no OS underneath)
   was being linked into the HOST test binary too, producing a
   corrupted executable that segfaulted immediately on startup with
   zero output — nothing printed, not even "running N tests". Fix:
   scope it to the custom target only, via `[target.x86_64-os]` in
   `.cargo/config.toml` (the target's JSON filename, sans extension,
   is what cargo uses as the config section name for a custom target).

## This development environment's toolchain workaround
This project's CI (`.github/workflows/*.yml`) and any environment with
normal internet access should use the standard path above. The sandbox
this kernel was originally developed and boot-tested in could not
reach `rustup`'s own domain (outside its network allowlist), so it
used Ubuntu's apt-packaged `rustc`/`cargo`/`rust-src` instead — a
"stable"-labeled release build. Consequences, recorded so a future
contributor hitting the same environment isn't surprised:

1. **`-Z` flags require `RUSTC_BOOTSTRAP=1`:** apt's `rustc` refuses
   unstable flags on a stable-labeled binary without it. This is
   Rust's own documented bootstrapping escape hatch (used by rustc's
   own build system), not an unofficial hack:
   ```bash
   cd kernel
   RUSTC_BOOTSTRAP=1 cargo build --release -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem
   RUSTC_BOOTSTRAP=1 cargo test --target x86_64-unknown-linux-gnu --bin neoastrenos-kernel
   ```
2. **apt's `rust-src` package ships without a `Cargo.lock`** for the
   `core`/`alloc` workspace (rustup's `rust-src` component includes
   one; the distro package does not). One-time fix, needed once per
   machine set up this way:
   ```bash
   cd /usr/lib/rustlib/src/rust
   RUSTC_BOOTSTRAP=1 cargo generate-lockfile
   ```
3. **`rust-lld`, as apt installs it, is a broken symlink** (points at
   a `lld-17` binary the `rustc`/`libstd-rust-dev` packages don't
   actually ship). `toolchain/x86_64-os.json` points `linker` at
   `ld.lld` instead (installed via `apt-get install lld`), a real,
   working lld binary — functionally the same linker, just not the
   copy rustc bundles internally.

## Boot-testing the kernel

The kernel is not runnable on its own — it is loaded and jumped to by
`boot/` (Phase 2, frozen). Full end-to-end test:

```bash
# 1. Build the (frozen, unmodified) bootloader
cd boot && make && cd ..

# 2. Build the kernel
cd kernel && RUSTC_BOOTSTRAP=1 cargo build --release -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem && cd ..
# (or: cargo +nightly build --release -Z build-std=..., if using rustup)

# 3. Assemble the ESP
mkdir -p build/esp/EFI/BOOT
cp build/BOOTX64.EFI build/esp/EFI/BOOT/BOOTX64.EFI
cp boot/testdata/BOOTINFO.TXT build/esp/BOOTINFO.TXT
cp kernel/target/x86_64-os/release/neoastrenos-kernel build/esp/KERNEL.ELF
cp /usr/share/OVMF/OVMF_CODE_4M.fd build/OVMF_CODE.fd
cp /usr/share/OVMF/OVMF_VARS_4M.fd build/OVMF_VARS.fd

# 4. Boot it
cd build
qemu-system-x86_64 -machine q35 -m 256M \
  -drive if=pflash,format=raw,readonly=on,file=OVMF_CODE.fd \
  -drive if=pflash,format=raw,file=OVMF_VARS.fd \
  -drive format=raw,file=fat:rw:esp \
  -serial stdio -display none -no-reboot
```

Expected final output ends with:
```
[OK] Physical frame allocator initialized.
  Total frames : 0x...
  Free frames  : 0x...

Running frame allocator boot self-test...
  [OK] Free frames after init: 0x...
  [OK] 16 allocations, all frames distinct.
  [OK] frames_free() bookkeeping correct after allocation.
  [OK] All 16 frames freed; frames_free() returned to its original value.
Frame allocator boot self-test: ALL CHECKS PASSED.

MEMORY MANAGER SUBSYSTEM: physical frame allocator verified.
[OK] Virtual memory manager initialized (EFER.NXE set, reusing the bootloader's existing PML4 at 0x...).

Running virtual memory manager boot self-test...
  [OK] translate() correctly resolves an address inside the bootloader's existing higher-half (2 MiB huge-page) kernel mapping.
  [OK] map() succeeded for a fresh page in the (previously unmapped) kernel heap region.
  [OK] Stored permission flags match exactly what map() was asked for.
  [OK] Write-then-read-back through the new mapping round-tripped correctly.
  [OK] translate() reports the correct physical frame for the new mapping.
  [OK] map() correctly rejects an already-mapped address.
  [OK] unmap() succeeded; translate() now correctly reports no mapping.
  [OK] unmap() correctly rejects an already-unmapped address.
Virtual memory manager boot self-test: ALL CHECKS PASSED.

MEMORY MANAGER SUBSYSTEM: virtual memory manager verified.
[OK] Frame allocator and VMM published to the kernel heap's global handles.

Running kernel heap allocator boot self-test...
  [OK] Box<u64> allocated in the correct heap region, value round-tripped.
  [OK] 100 small allocations, all distinct addresses, all values intact.
  [OK] 20,000-element Vec<u32> (multiple growth cycles) built and checksum verified.
  [OK] Repeated large alloc/free cycles completed (freed space is being reused).
Kernel heap allocator boot self-test: ALL CHECKS PASSED.

MEMORY MANAGER SUBSYSTEM: kernel heap allocator verified.
MEMORY MANAGER SUBSYSTEM COMPLETE: frame allocator, virtual memory
manager, and kernel heap allocator all implemented and verified.
Halting.
```
