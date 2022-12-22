use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uefi_run::*;

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

        // Create EFI executable
        if args.boot {
            image.copy_host_file(&args.efi_exe, "EFI/Boot/BootX64.efi")
        } else {
            image.copy_host_file(&args.efi_exe, "run.efi")
        }
        .expect("Failed to copy EFI executable");

        // Create startup.nsh
        image
            .set_file_contents("startup.nsh", DEFAULT_STARTUP_NSH)
            .expect("Failed to write startup script");

        // Create user provided additional files
        for (outer, inner) in args.parse_add_file_args().map(|x| x.unwrap()) {
            // Copy the file into the image
            image
                .copy_host_file(outer, inner)
                .expect("Failed to copy user-defined file");
        }
    }

    let mut qemu_config = QemuConfig {
        qemu_path: args.qemu_path,
        bios_path: args.bios_path,
        drives: vec![QemuDriveConfig {
            file: image_file_path.to_str().unwrap().to_string(),
            media: "disk".to_string(),
            format: "raw".to_string(),
        }],
        ..Default::default()
    };
    qemu_config
        .additional_args
        .extend(args.qemu_args.iter().cloned());

    // Run qemu
    let mut qemu_process = qemu_config.run().expect("Failed to start qemu");

    // Wait for qemu to exit or signal.
    let mut qemu_exit_code;
    loop {
        qemu_exit_code = qemu_process.wait(Duration::from_millis(500));
        if qemu_exit_code.is_some() || terminating.load(Ordering::SeqCst) {
            break;
        }
    }

    // The above loop may have been broken by a signal
    if qemu_exit_code.is_none() {
        // In this case we wait for qemu to exit for one second
        qemu_exit_code = qemu_process.wait(Duration::from_secs(1));
    }

    // Qemu may still be running
    if qemu_exit_code.is_none() {
        // In this case we need to kill it
        qemu_process
            .kill()
            .or_else(|e| match e.kind() {
                // Not running anymore
                std::io::ErrorKind::InvalidInput => Ok(()),
                _ => Err(e),
            })
            .expect("Unable to kill qemu process");
        qemu_exit_code = qemu_process.wait(Duration::from_secs(1));
    }

    let exit_code = qemu_exit_code.expect("qemu should have exited by now but did not");
    std::process::exit(exit_code);
}
