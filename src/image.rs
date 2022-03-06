use super::*;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Handle to a FAT filesystem used as an EFI partition
pub struct EfiImage {
    fs: fatfs::FileSystem<fs::File>,
}

impl EfiImage {
    /// Create a new image at the given path
    pub fn new<P: AsRef<Path>>(path: P, size: u64) -> Result<Self> {
        // Create regular file and truncate it to size.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.set_len(size)?;

        // Create FAT fs and open it
        fatfs::format_volume(&file, fatfs::FormatVolumeOptions::new())?;
        let fs = fatfs::FileSystem::new(file, fatfs::FsOptions::new())?;

        Ok(Self { fs })
    }

    /// Add file to the image
    fn add_file<P: AsRef<Path>>(&mut self, path: P) -> Result<fatfs::File<'_, fs::File>> {
        let path = path.as_ref();
        let file_name = path
            .file_name()
            .ok_or_else(|| Error::msg("Invalid path"))?
            .to_str()
            .ok_or_else(|| Error::msg("Invalid filename encoding"))?;
        let mut dir = self.fs.root_dir();
        if let Some(dir_path) = path.parent() {
            for dir_path_component in dir_path.iter() {
                if dir_path_component == OsStr::new(&std::path::MAIN_SEPARATOR.to_string()) {
                    continue;
                }
                let dir_path_component = dir_path_component
                    .to_str()
                    .ok_or_else(|| Error::msg("Cannot convert path to string"))?;
                dir = dir.create_dir(dir_path_component)?;
            }
        }
        let mut file = dir.create_file(file_name)?;
        file.truncate()?;
        Ok(file)
    }

    /// Copy file from host filesystem to the image
    pub fn copy_host_file<P1: AsRef<Path>, P2: AsRef<Path>>(
        &mut self,
        src: P1,
        dst: P2,
    ) -> Result<()> {
        let file_contents = fs::read(src)?;
        let mut file = self.add_file(dst)?;
        file.write_all(&file_contents)?;
        Ok(())
    }

    /// Write file contents
    pub fn set_file_contents<P: AsRef<Path>, B: AsRef<[u8]>>(
        &mut self,
        path: P,
        contents: B,
    ) -> Result<()> {
        let mut file = self.add_file(path)?;
        file.write_all(contents.as_ref())?;
        Ok(())
    }
}
