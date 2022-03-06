use clap::Parser;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uefi_run::*;
use wait_timeout::ChildExt;

#[derive(Parser, Debug, PartialEq)]
#[clap(
    version,
    author,
    about,
    trailing_var_arg = true,
    dont_delimit_trailing_values = true,
)]
struct Args {
    /// Bost image
    #[clap(long, short = 'b', default_value = "OVMF.fd")]
    bios_path: String,
    /// Path to qemu executable
    #[clap(long, short = 'q', default_value = "qemu-system-x86_64")]
    qemu_path: String,
    /// Size of the image in MiB
    #[clap(long, short = 's', default_value_t = 10)]
    size: u64,
    /// Additional files to be added to the efi image
    ///
    /// Additional files to be added to the efi image. If no inner location is provided, it will
    /// default to the root of the image with the same name as the provided file.
    #[clap(long, short = 'f')]
    add_file: Vec<String>,
    /// EFI Executable
    efi_exe: String,
    /// Additional arguments for qemu
    qemu_args: Vec<String>,
}

fn main() {
    // Parse command line
    let args = Args::parse();

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
        let mut image =
            EfiImage::new(&image_file_path, args.size * 0x10_0000).expect("Failed to create image");

        // Create run.efi
        image
            .copy_host_file(&args.efi_exe, "run.efi")
            .expect("Failed to copy EFI executable");

        // Create startup.nsh
        image
            .set_file_contents("startup.nsh", include_bytes!("startup.nsh"))
            .expect("Failed to write startup script");

        // Create user provided additional files
        for file in args.add_file {
            // Split the argument to get the inner and outer files
            let (outer, inner) = file
                .split_once(':')
                .map(|(x, y)| (PathBuf::from(x), PathBuf::from(y)))
                .unwrap_or_else(|| {
                    let outer = PathBuf::from(&file);
                    let inner = PathBuf::from(&file)
                        .file_name()
                        .expect("Invalid --add-file argument")
                        .into();
                    (outer, inner)
                });
            // Copy the file into the image
            image
                .copy_host_file(outer, inner)
                .expect("Failed to copy user-defined file");
        }
    }

    let mut qemu_args = vec![
        "-drive".into(),
        format!(
            "file={},index=0,media=disk,format=raw",
            image_file_path.display()
        ),
        "-bios".into(),
        args.bios_path,
        "-net".into(),
        "none".into(),
    ];
    qemu_args.extend(args.qemu_args.iter().map(|x| x.into()));

    // Run qemu.
    let mut child = Command::new(args.qemu_path)
        .args(qemu_args)
        .spawn()
        .expect("Failed to start qemu");

    // Wait for qemu to exit or signal.
    let mut qemu_exit_code;
    loop {
        qemu_exit_code = wait_qemu(&mut child, Duration::from_millis(500));
        if qemu_exit_code.is_some() || terminating.load(Ordering::SeqCst) {
            break;
        }
    }

    // The above loop may have been broken by a signal
    if qemu_exit_code.is_none() {
        // In this case we wait for qemu to exit for one second
        qemu_exit_code = wait_qemu(&mut child, Duration::from_secs(1));
    }

    // Qemu may still be running
    if qemu_exit_code.is_none() {
        // In this case we need to kill it
        child
            .kill()
            .or_else(|e| match e.kind() {
                // Not running anymore
                std::io::ErrorKind::InvalidInput => Ok(()),
                _ => Err(e),
            })
            .expect("Unable to kill qemu process");
        qemu_exit_code = wait_qemu(&mut child, Duration::from_secs(1));
    }

    let exit_code = qemu_exit_code.expect("qemu should have exited by now but did not");
    std::process::exit(exit_code);
}

/// Wait for the process to exit for `duration`.
///
/// Returns `true` if the process exited and false if the timeout expired.
fn wait_qemu(child: &mut Child, duration: Duration) -> Option<i32> {
    child
        .wait_timeout(duration)
        .expect("Failed to wait on child process")
        .map(|exit_status| exit_status.code().unwrap_or(0))
}
