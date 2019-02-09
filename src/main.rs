extern crate clap;
extern crate fatfs;

use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// Create a file and open it for reading and writing.
fn open_file<P: AsRef<Path>>(path: P) -> std::fs::File {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
        .unwrap()
}

/// Create a FAT filesystem image.
fn format_image<P: AsRef<Path>>(path: P) {
    let file = open_file(path);
    file.set_len(10 * 1024 * 1024).unwrap();
    fatfs::format_volume(file, fatfs::FormatVolumeOptions::new()).unwrap();
}

/// Create a bootable UEFI partition from an empty FAT image.
fn create_image<P: AsRef<Path>>(img_path: P, efi_exe_path: P) {
    format_image(&img_path);
    let fs = fatfs::FileSystem::new(open_file(&img_path), fatfs::FsOptions::new()).unwrap();

    // Create run.efi
    let efi_exe_contents = std::fs::read(efi_exe_path).unwrap();
    let mut run_efi = fs.root_dir().create_file("run.efi").unwrap();
    run_efi.truncate().unwrap();
    run_efi.write_all(&efi_exe_contents).unwrap();

    // Create startup.nsh
    let mut startup_nsh = fs.root_dir().create_file("startup.nsh").unwrap();
    startup_nsh.truncate().unwrap();
    startup_nsh.write_all(include_bytes!("startup.nsh")).unwrap();
}

fn main() {
    let matches = clap::App::new("uefi-run")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Richard Wiedenhöft <richard@wiedenhoeft.xyz>")
        .about("Runs UEFI executables in qemu.")
        .arg(clap::Arg::with_name("efi_exe")
            .value_name("FILE")
            .required(true)
            .help("EFI executable")
        )
        .arg(clap::Arg::with_name("bios_file")
             .value_name("bios_file")
             .required(false)
             .help("BIOS image (default = /usr/share/ovmf/OVMF.fd)")
             .short("b")
             .long("bios")
         )
        .get_matches();

    // Parse options
    let efi_exe = matches.value_of("efi_exe").unwrap();
    let bios_file = matches.value_of("bios_file").unwrap_or("/usr/share/ovmf/OVMF.fd");

    let img_path = {
        let mut path_buf = std::env::temp_dir();
        path_buf.push(format!("uefi-run-img.{}.fat", std::process::id()));
        path_buf
    };
    create_image(&img_path, &PathBuf::from(&efi_exe));

    // Run qemu and wait for it to terminate.
    let ecode = Command::new("/usr/bin/qemu-system-x86_64")
        .args(&[
            "-drive".into(), format!("file={},index=0,media=disk,format=raw", img_path.display()),
            "-bios".into(), format!("{}", bios_file),
            "-net".into(), "none".into(),
        ])
        .spawn()
        .expect("Failed to start qemu")
        .wait()
        .expect("Failed to wait on qemu")
        ;
    if !ecode.success() {
        println!("qemu execution failed");
    }

    // Delete the image file.
    std::fs::remove_file(&img_path).unwrap();
}
