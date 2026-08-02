# Building and Testing the Kernel

## Standard path (rustup available)
```bash
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
cd kernel
cargo +nightly build --release
```
`.cargo/config.toml` already points `cargo` at `../toolchain/x86_64-os.json`
and enables `build-std = ["core", "alloc"]`, so a plain `cargo build`
(under the nightly toolchain `rust-toolchain.toml` selects) is enough —
no extra flags needed.

## This development environment's workaround
This project's CI (`.github/workflows/build.yml`) and any environment
with normal internet access should use the standard path above.
The sandbox this kernel was originally developed and boot-tested in
could not reach `rustup`'s own domain (outside its network allowlist),
so it used Ubuntu's apt-packaged `rustc`/`cargo`/`rust-src` instead —
a "stable"-labeled release build. Two consequences, both worth
recording so a future contributor hitting the same environment isn't
surprised by them:

1. **`build-std` requires `RUSTC_BOOTSTRAP=1`:** apt's `rustc` refuses
   the unstable `-Z build-std` machinery on a stable-labeled binary
   without it. This is Rust's own documented bootstrapping escape
   hatch (used by rustc's own build system), not an unofficial hack:
   ```bash
   cd kernel
   RUSTC_BOOTSTRAP=1 cargo build --release
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
   `ld.lld` instead (installed via `apt-get install lld`), which is a
   real, working lld binary — functionally the same linker, just not
   the copy rustc bundles internally.

## Boot-testing the kernel

The kernel is not runnable on its own — it is loaded and jumped to by
`boot/` (Phase 2, frozen). Full end-to-end test:

```bash
# 1. Build the (frozen, unmodified) bootloader
cd boot && make && cd ..

# 2. Build the kernel
cd kernel && RUSTC_BOOTSTRAP=1 cargo build --release && cd ..
# (or: cargo +nightly build --release, if using rustup)

# 3. Assemble the ESP
mkdir -p build/esp/EFI/BOOT
cp build/BOOTX64.EFI build/esp/EFI/BOOT/BOOTX64.EFI
cp boot/testdata/BOOTINFO.TXT build/esp/BOOTINFO.TXT
cp kernel/target/x86_64-os/release/xyronos-kernel build/esp/KERNEL.ELF
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
PHASE 3 SKELETON: kernel_main reached, BootInfo valid, Rust toolchain verified.
Memory manager, interrupts, scheduler: not yet implemented (see docs/kernel/).
Halting.
```
