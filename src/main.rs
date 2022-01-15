use uefi_run_lib::Qemu;

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
                .default_value("OVMF.fd")
                .help("BIOS image")
                .short("b")
                .long("bios"),
        )
        .arg(
            clap::Arg::with_name("qemu_path")
                .value_name("qemu_path")
                .default_value("qemu-system-x86_64")
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
            clap::Arg::with_name("add_files")
                .value_name("location_on_disk>:<location_within_image")
                .required(false)
                .help("Additional files to be added to the efi image")
                .long_help(
                    "Additional files to be added to the efi image\n\
                     If no inner location is provided, it will default\n\
                     to the root of the image with the same name as the provided file",
                )
                .multiple(true)
                .short("f")
                .long("add-file")
                .number_of_values(1),
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
    let user_qemu_args: Vec<_> = matches
        .values_of_os("qemu_args")
        .unwrap_or_default()
        .collect();
    let additional_files: Vec<_> = matches.values_of("add_files").unwrap_or_default().collect();

    let qemu_exit_code = Qemu {
        efi_exe,
        bios_path,
        qemu_path,
        size,
        user_qemu_args: &user_qemu_args,
        additional_files: &additional_files,
    }
    .run();

    let exit_code = qemu_exit_code.expect("qemu should have exited by now but did not");
    std::process::exit(exit_code);
}
