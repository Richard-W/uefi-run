# uefi-run [![Latest Version]][crates.io] [![Build Status]][travis]

[Build Status]: https://travis-ci.org/Richard-W/uefi-run.svg?branch=master
[travis]: https://travis-ci.org/Richard-W/uefi-run
[Latest Version]: https://img.shields.io/crates/v/uefi-run.svg
[crates.io]: https://crates.io/crates/uefi-run

**Directly run UEFI applications in qemu**

---

This helper application takes an EFI executable, builds a FAT filesystem around
it, adds a startup script and runs qemu to run the executable.

It does not require root permissions since it uses the [fatfs](https://crates.io/crates/fatfs)
crate to build the filesystem image directly without involving `mkfs`, `mount`,
etc.

## Installation

### Snap

uefi-run can be installed from the snapstore:
```bash
snap install --edge uefi-run
```
The confinement of this snap is somewhat strict. It can only access non-hidden files in the user's
home directory. Also it has no network access.

### Cargo

You can install cargo and rust using the rustup tool:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After cargo has been installed you can build and install uefi-run:
```bash
cargo install uefi-run
```
