extern crate clap;
extern crate fatfs;
extern crate tempfile;

use std::io::Write;
use std::process::Command;

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

    // Create temporary image file
    let image_file = tempfile::NamedTempFile::new()
        .expect("Temporary image creation failed");
    // Truncate image to 10MiB
    image_file.as_file().set_len(10 * 0x10_0000)
        .expect("Truncating image file failed");
    // Format file as FAT
    fatfs::format_volume(&image_file, fatfs::FormatVolumeOptions::new())
        .expect("Formatting image file failed");

    {
        let fs = fatfs::FileSystem::new(&image_file, fatfs::FsOptions::new()).unwrap();

        // Create run.efi
        let efi_exe_contents = std::fs::read(efi_exe).unwrap();
        let mut run_efi = fs.root_dir().create_file("run.efi").unwrap();
        run_efi.truncate().unwrap();
        run_efi.write_all(&efi_exe_contents).unwrap();

        // Create startup.nsh
        let mut startup_nsh = fs.root_dir().create_file("startup.nsh").unwrap();
        startup_nsh.truncate().unwrap();
        startup_nsh.write_all(include_bytes!("startup.nsh")).unwrap();
    }

    // Run qemu and wait for it to terminate.
    let ecode = Command::new("/usr/bin/qemu-system-x86_64")
        .args(&[
            "-drive".into(), format!("file={},index=0,media=disk,format=raw", image_file.path().display()),
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
}
