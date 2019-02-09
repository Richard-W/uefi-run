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
