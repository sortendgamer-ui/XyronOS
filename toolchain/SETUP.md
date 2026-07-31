# Toolchain Setup

This OS is built with two cross-compilers targeting freestanding x86_64 —
neither compiler may link against a host OS's standard library or runtime.

## 1. Rust cross-compiler (kernel core)

```bash
# Use the nightly toolchain — building custom targets requires
# the unstable `build-std` feature.
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
rustup default nightly
```

We don't use a built-in Rust target triple (like `x86_64-unknown-linux-gnu`)
because every built-in triple assumes an OS underneath it. Instead we ship
our own target spec file, `x86_64-os.json` (added in Phase 3 alongside the
first kernel source), and build with:

```bash
cargo build -Z build-std=core,alloc --target toolchain/x86_64-os.json
```

## 2. C cross-compiler (drivers)

Build `x86_64-elf-gcc` from source (no distro packages — most Linux distro
GCC is tied to glibc/host ABI, which we cannot depend on):

```bash
# Requires: build-essential, bison, flex, libgmp-dev, libmpc-dev, libmpfr-dev
mkdir -p ~/opt/cross-src && cd ~/opt/cross-src
curl -O https://ftp.gnu.org/gnu/binutils/binutils-2.42.tar.gz
curl -O https://ftp.gnu.org/gnu/gcc/gcc-13.2.0/gcc-13.2.0.tar.gz
# ... standard OSDev cross-compiler build steps (binutils first, then gcc)
# targeting x86_64-elf, installed to ~/opt/cross/bin
export PATH="$HOME/opt/cross/bin:$PATH"
```

## 2b. PE32+ cross-compiler (bootloader specifically)

UEFI applications must be PE32+ binaries, not ELF — this is a firmware
requirement, independent of our C11/no-existing-source-code rule. We use
mingw-w64 purely as a freestanding PE32+ code generator (no CRT, no libc
linked in — see `boot/Makefile`):

```bash
sudo apt-get install mingw-w64
x86_64-w64-mingw32-gcc --version   # verify
```

## 3. Bootloader assembler

```bash
sudo apt-get install nasm    # or: brew install nasm
```

## 4. Emulator for testing (no real hardware needed yet)

```bash
sudo apt-get install qemu-system-x86 ovmf   # ovmf = open-source UEFI firmware
```

## Verifying your setup

```bash
nasm -v
x86_64-elf-gcc --version
rustc +nightly --version
qemu-system-x86_64 --version
ls /usr/share/ovmf/OVMF.fd   # UEFI firmware image QEMU will boot
```

If all four print a version and OVMF.fd exists, the environment is ready
for Phase 2 (Bootloader).
