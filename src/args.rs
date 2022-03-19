use super::*;
use clap::Parser;
use std::path::PathBuf;

/// Command line arguments for uefi-run
#[derive(Parser, Debug, Default, PartialEq)]
#[clap(
    version,
    author,
    about,
    trailing_var_arg = true,
    dont_delimit_trailing_values = true
)]
pub struct Args {
    /// Bios image
    #[clap(long, short = 'b', default_value = "OVMF.fd")]
    pub bios_path: String,
    /// Path to qemu executable
    #[clap(long, short = 'q', default_value = "qemu-system-x86_64")]
    pub qemu_path: String,
    /// Size of the image in MiB
    #[clap(long, short = 's', default_value_t = 10)]
    pub size: u64,
    /// Additional files to be added to the efi image
    ///
    /// Additional files to be added to the efi image. If no inner location is provided, it will
    /// default to the root of the image with the same name as the provided file.
    #[clap(long, short = 'f')]
    pub add_file: Vec<String>,
    /// EFI Executable
    pub efi_exe: String,
    /// Additional arguments for qemu
    pub qemu_args: Vec<String>,
}

impl Args {
    /// Parse `--add-file` arguments into `(outer, inner)` tuples of `PathBuf`
    pub fn parse_add_file_args(&self) -> impl Iterator<Item = Result<(PathBuf, PathBuf)>> + '_ {
        self.add_file.iter().map(|file| {
            // Split the argument to get the inner and outer files
            file.split_once(':')
                .map(|(x, y)| Ok((PathBuf::from(x), PathBuf::from(y))))
                .unwrap_or_else(|| {
                    let outer = PathBuf::from(&file);
                    let inner = PathBuf::from(&file)
                        .file_name()
                        .ok_or_else(|| Error::msg("Invalid --add-file argument"))?
                        .into();
                    Ok((outer, inner))
                })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_add_file_args() {
        let mut args = Args::default();
        args.add_file = vec![
            "/full/path/to/outer:/full/path/to/inner".to_string(),
            "/full/path/to/outer:inner".to_string(),
            "outer:inner".to_string(),
            "/full/path/to/outer".to_string(),
            "outer".to_string(),
        ];
        #[rustfmt::skip]
        let expected = vec![
            (PathBuf::from("/full/path/to/outer"), PathBuf::from("/full/path/to/inner")),
            (PathBuf::from("/full/path/to/outer"), PathBuf::from("inner")),
            (PathBuf::from("outer"), PathBuf::from("inner")),
            (PathBuf::from("/full/path/to/outer"), PathBuf::from("outer")),
            (PathBuf::from("outer"), PathBuf::from("outer")),
        ];
        let actual = args
            .parse_add_file_args()
            .map(|x| x.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
