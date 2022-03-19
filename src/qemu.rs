use super::*;
use std::process::{Child, Command};
use std::time::Duration;
use wait_timeout::ChildExt;

/// Qemu run configuration
#[derive(Debug, Clone)]
pub struct QemuConfig {
    pub qemu_path: String,
    pub bios_path: String,
    pub drives: Vec<QemuDriveConfig>,
    pub additional_args: Vec<String>,
}

impl Default for QemuConfig {
    fn default() -> Self {
        Self {
            qemu_path: "qemu-system-x86_64".to_string(),
            bios_path: "OVMF.fd".to_string(),
            drives: Vec::new(),
            additional_args: vec!["-net".to_string(), "none".to_string()],
        }
    }
}

impl QemuConfig {
    /// Run an instance of qemu with the given config
    pub fn run(&self) -> Result<QemuProcess> {
        let mut args = vec!["-bios".to_string(), self.bios_path.clone()];
        for (index, drive) in self.drives.iter().enumerate() {
            args.push("-drive".to_string());
            args.push(format!(
                "file={},index={},media={},format={}",
                drive.file, index, drive.media, drive.format
            ));
        }
        args.extend(self.additional_args.iter().cloned());

        let child = Command::new(&self.qemu_path).args(args).spawn()?;
        Ok(QemuProcess { child })
    }
}

/// Qemu drive configuration
#[derive(Debug, Clone)]
pub struct QemuDriveConfig {
    pub file: String,
    pub media: String,
    pub format: String,
}

impl QemuDriveConfig {
    pub fn new(file: &str, media: &str, format: &str) -> Self {
        Self {
            file: file.to_string(),
            media: media.to_string(),
            format: format.to_string(),
        }
    }
}

pub struct QemuProcess {
    child: Child,
}

impl QemuProcess {
    /// Wait for the process to exit for `duration`.
    ///
    /// Returns `true` if the process exited and false if the timeout expired.
    pub fn wait(&mut self, duration: Duration) -> Option<i32> {
        self.child
            .wait_timeout(duration)
            .expect("Failed to wait on child process")
            .map(|exit_status| exit_status.code().unwrap_or(0))
    }

    /// Kill the process.
    pub fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }
}
