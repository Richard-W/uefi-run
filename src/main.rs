extern crate clap;
extern crate fatfs;

use std::path::Path;
use std::io::Write;

fn open_file<P: AsRef<Path>>(path: P) -> std::fs::File {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
        .unwrap()
}

fn format_image<P: AsRef<Path>>(path: P) {
    let file = open_file(path);
    file.set_len(10 * 1024 * 1024).unwrap();
    fatfs::format_volume(file, fatfs::FormatVolumeOptions::new()).unwrap();
}

fn open_image<P: AsRef<Path>>(path: P) -> fatfs::FileSystem<std::fs::File> {
    let file = open_file(path);
    fatfs::FileSystem::new(file, fatfs::FsOptions::new()).unwrap()
}

fn main() {
    let matches = clap::App::new("uefi-run")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Richard Wiedenhöft <richard@wiedenhoeft.xyz>")
        .about("Runs UEFI executables in qemu.")
        .arg(clap::Arg::with_name("FILE")
            .required(true)
            .help("The file that is executed")
        )
        .get_matches();

    let efi_file_contents = std::fs::read(matches.value_of("FILE").unwrap()).unwrap();

    let img_path = {
        let mut path_builder = std::env::temp_dir();
        path_builder.push(format!("uefi-run-img.{}.fat", std::process::id()));
        path_builder
    };
    format_image(&img_path);
    let fs = open_image(&img_path);

    // Create run.efi
    let mut run_efi = fs.root_dir().create_file("run.efi").unwrap();
    run_efi.truncate().unwrap();
    run_efi.write_all(&efi_file_contents).unwrap();

    std::fs::remove_file(&img_path).unwrap();
}
