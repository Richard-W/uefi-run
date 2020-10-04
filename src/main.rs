extern crate clap;
extern crate ctrlc;
extern crate fatfs;
extern crate tempfile;
extern crate wait_timeout;

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use wait_timeout::ChildExt;

fn main() {
    let matches = clap::App::new("uefi-run")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Richard Wiedenhöft <richard@wiedenhoeft.xyz>")
        .about("Runs UEFI executables in qemu.")
        .setting(clap::AppSettings::TrailingVarArg)
        .setting(clap::AppSettings::DontDelimitTrailingValues)
        .arg(
            clap::Arg::with_name("efi_exe")
                .value_name("FILE")
                .required(true)
                .help("EFI executable"),
        )
        .arg(
            clap::Arg::with_name("bios_path")
                .value_name("bios_path")
                .default_value("/usr/share/ovmf/OVMF.fd")
                .help("BIOS image")
                .short("b")
                .long("bios"),
        )
        .arg(
            clap::Arg::with_name("qemu_path")
                .value_name("qemu_path")
                .default_value("/usr/bin/qemu-system-x86_64")
                .help("Path to qemu executable")
                .short("q")
                .long("qemu"),
        )
        .arg(
            clap::Arg::with_name("size")
                .value_name("size")
                .default_value("10")
                .help("Size of the image in MiB")
                .short("s")
                .long("size"),
        )
        .arg(
            clap::Arg::with_name("qemu_args")
                .value_name("qemu_args")
                .required(false)
                .help("Additional arguments for qemu")
                .multiple(true),
        )
        .get_matches();

    // Parse options
    let efi_exe = matches.value_of("efi_exe").unwrap();
    let bios_path = matches.value_of("bios_path").unwrap();
    let qemu_path = matches.value_of("qemu_path").unwrap();
    let size: u64 = matches
        .value_of("size")
        .map(|v| v.parse().expect("Failed to parse --size argument"))
        .unwrap();
    let user_qemu_args = matches.values_of("qemu_args").unwrap_or_default();

    // Install termination signal handler. This ensures that the destructor of
    // `temp_dir` which is constructed in the next step is really called and
    // the files are cleaned up properly.
    let terminating = Arc::new(AtomicBool::new(false));
    {
        let term = terminating.clone();
        ctrlc::set_handler(move || {
            println!("uefi-run terminating...");
            // Tell the main thread to stop waiting.
            term.store(true, Ordering::SeqCst);
        })
        .expect("Error setting termination handler");
    }

    // Create temporary dir for the image file.
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let temp_dir_path = PathBuf::from(temp_dir.path());

    // Path to the image file
    let image_file_path = {
        let mut path_buf = temp_dir_path;
        path_buf.push("image.fat");
        path_buf
    };

    {
        // Create image file
        let image_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&image_file_path)
            .expect("Image file creation failed");
        // Truncate image to `size` MiB
        image_file
            .set_len(size * 0x10_0000)
            .expect("Truncating image file failed");
        // Format file as FAT
        fatfs::format_volume(&image_file, fatfs::FormatVolumeOptions::new())
            .expect("Formatting image file failed");

        // Open the FAT fs.
        let fs = fatfs::FileSystem::new(&image_file, fatfs::FsOptions::new())
            .expect("Failed to open filesystem");

        // Create run.efi
        let efi_exe_contents = std::fs::read(efi_exe).unwrap();
        let mut run_efi = fs.root_dir().create_file("run.efi").unwrap();
        run_efi.truncate().unwrap();
        run_efi.write_all(&efi_exe_contents).unwrap();

        // Create startup.nsh
        let mut startup_nsh = fs.root_dir().create_file("startup.nsh").unwrap();
        startup_nsh.truncate().unwrap();
        startup_nsh
            .write_all(include_bytes!("startup.nsh"))
            .unwrap();
    }

    let mut qemu_args = vec![
        "-drive".into(),
        format!(
            "file={},index=0,media=disk,format=raw",
            image_file_path.display()
        ),
        "-bios".into(),
        bios_path.into(),
        "-net".into(),
        "none".into(),
    ];
    qemu_args.extend(user_qemu_args.map(|x| x.into()));

    // Run qemu.
    let mut child = Command::new(qemu_path)
        .args(qemu_args)
        .spawn()
        .expect("Failed to start qemu");

    // Wait for qemu to exit or signal.
    let mut child_terminated;
    loop {
        child_terminated = wait_qemu(&mut child, Duration::from_millis(500));
        if child_terminated || terminating.load(Ordering::SeqCst) {
            break;
        }
    }

    // If uefi-run received a signal we still need the child to exit.
    if !child_terminated {
        child_terminated = wait_qemu(&mut child, Duration::from_secs(1));
        if !child_terminated {
            match child.kill() {
                // Kill succeeded
                Ok(_) => assert!(wait_qemu(&mut child, Duration::from_secs(1))),
                Err(e) => {
                    match e.kind() {
                        // Not running anymore
                        std::io::ErrorKind::InvalidInput => {
                            assert!(wait_qemu(&mut child, Duration::from_secs(1)))
                        }
                        // Other error
                        _ => panic!("Not able to kill child process: {:?}", e),
                    }
                }
            }
        }
    }
}

/// Wait for the process to exit for `duration`.
///
/// Returns `true` if the process exited and false if the timeout expired.
fn wait_qemu(child: &mut Child, duration: Duration) -> bool {
    let wait_result = child
        .wait_timeout(duration)
        .expect("Failed to wait on child process");
    match wait_result {
        None => {
            // Child still alive.
            false
        }
        Some(exit_status) => {
            // Child exited.
            if !exit_status.success() {
                match exit_status.code() {
                    Some(code) => println!("qemu exited with status {}", code),
                    None => println!("qemu exited unsuccessfully"),
                }
            }
            true
        }
    }
}
