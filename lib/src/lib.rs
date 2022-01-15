use std::ffi::OsStr;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use wait_timeout::ChildExt;

pub struct Qemu<'a> {
    pub efi_exe: &'a str,
    pub bios_path: &'a str,
    pub qemu_path: &'a str,
    pub size: u64,
    pub user_qemu_args: &'a [&'a OsStr],
    pub additional_files: &'a [&'a str],
}

impl<'a> Qemu<'a> {
    pub fn run(&self) -> Option<i32> {
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
                .set_len(self.size * 0x10_0000)
                .expect("Truncating image file failed");
            // Format file as FAT
            fatfs::format_volume(&image_file, fatfs::FormatVolumeOptions::new())
                .expect("Formatting image file failed");

            // Open the FAT fs.
            let fs = fatfs::FileSystem::new(&image_file, fatfs::FsOptions::new())
                .expect("Failed to open filesystem");

            // Create run.efi
            let efi_exe_contents = std::fs::read(self.efi_exe).unwrap();
            let mut run_efi = fs.root_dir().create_file("run.efi").unwrap();
            run_efi.truncate().unwrap();
            run_efi.write_all(&efi_exe_contents).unwrap();

            // Create startup.nsh
            let mut startup_nsh = fs.root_dir().create_file("startup.nsh").unwrap();
            startup_nsh.truncate().unwrap();
            startup_nsh
                .write_all(include_bytes!("startup.nsh"))
                .unwrap();

            // Create user provided additional files
            for file in self.additional_files {
                // Get a reference to the root of the image file
                let mut current_fs_dir = fs.root_dir();
                // Save a reference to the origional argument
                let orig_file = file;

                // Split the argument to get the inner and outer files
                let mut file = file.split(':');
                // Get the path to the real file on the host system
                let outer = PathBuf::from(
                    file.next()
                        .expect(&format!("Invalid --add-file argument: \"{}\"", orig_file)),
                );
                // Get the path to the file/dir to write the file to within the FAT fs drive
                let inner = match file.next_back() {
                    Some(path) => PathBuf::from(path),
                    None => outer.clone(),
                };
                // Make sure the argument was actually well-formed (ie. no additional ':' segments)
                if file.next().is_some() {
                    panic!("Invalid --add-file argument: \"{}\"", orig_file)
                }

                // Convert the inner to an iterator so we can get the components
                let mut inner = inner.iter();

                // Get the inner filename
                let inner_file = inner
                    .next_back()
                    .expect(&format!("Invalid --add-file argument: \"{}\"", orig_file))
                    .to_str()
                    .expect(&format!("Invalid --add-file argument: \"{}\"", orig_file));

                // Step through each dir within the inner path
                for path in inner {
                    // The only component that would be just a `MAIN_SEPARATOR` is the RootDir
                    // component if provided, which we can just ignore as we always build from
                    // the root for the inner file anyway
                    if path == OsStr::new(&std::path::MAIN_SEPARATOR.to_string()) {
                        continue;
                    }

                    // Convert each path component into a &str because `fatfs` can't handle &OsStr's
                    let path = path
                        .to_str()
                        .expect(&format!("Invalid --add-file argument: \"{}\"", orig_file));

                    // Create the path within the image and set it as the current dir
                    current_fs_dir = current_fs_dir.create_dir(path).unwrap();
                }

                // Create (or open) the file within the image
                let mut user_file = current_fs_dir.create_file(inner_file).unwrap();

                // Read in the outer file
                let data = std::fs::read(outer).expect(&format!(
                    "Invalid --add-file argument - Failed to read outer file: \"{}\"",
                    orig_file
                ));

                // Erase the file inside the image (if it exists)
                // This means that any files that have already been placed in the image
                // (due to earlier command line arguments) will be overwritten.
                user_file.truncate().unwrap();
                // Write the file to the image
                user_file.write_all(&data).unwrap();
            }
        }

        // Create qemu command.
        let mut cmd = Command::new(self.qemu_path);
        cmd.args(&[
            "-drive",
            &format!(
                "file={},index=0,media=disk,format=raw",
                image_file_path.display()
            ),
            "-bios",
            self.bios_path,
            "-net",
            "none",
        ]);
        cmd.args(self.user_qemu_args);

        // Run qemu.
        let mut child = cmd.spawn().expect("Failed to start qemu");

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

        qemu_exit_code
    }
}

/// Wait for the process to exit for `duration`.
///
/// Returns `true` if the process exited and false if the timeout expired.
fn wait_qemu(child: &mut Child, duration: Duration) -> Option<i32> {
    let wait_result = child
        .wait_timeout(duration)
        .expect("Failed to wait on child process");
    match wait_result {
        None => {
            // Child still alive.
            None
        }
        Some(exit_status) => Some(exit_status.code().unwrap_or(0)),
    }
}
