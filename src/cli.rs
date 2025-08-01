use clap::{Arg, Command};

pub fn get_cli_matches() -> clap::ArgMatches {
    Command::new("glint")
        .version("0.1.0")
        .author("John Freeman")
        .about("A local-only, git-friendly scratchpad for testing API endpoints in your terminal.")
        .allow_external_subcommands(true)
        .subcommand(
            Command::new("vault")
                .about("Manage encrypted configuration files securely")
                .subcommand(
                    Command::new("create")
                        .about("Create a new, empty encrypted vault file")
                        .arg(
                            Arg::new("output")
                                .short('o')
                                .long("output")
                                .takes_value(true)
                                .help("Specify the name of the encrypted file"),
                        ),
                )
                .subcommand(
                    Command::new("encrypt")
                        .about("Encrypt an existing plain-text file into a vault")
                        .arg(
                            Arg::new("file")
                                .required(true)
                                .help("The plain-text file to encrypt"),
                        )
                        .arg(
                            Arg::new("delete")
                                .long("delete")
                                .takes_value(false)
                                .help("Delete the original plain-text file after encryption"),
                        )
                        .arg(
                            Arg::new("output")
                                .short('o')
                                .long("output")
                                .takes_value(true)
                                .help("Specify the name of the encrypted output file"),
                        ),
                )
                .subcommand(
                    Command::new("decrypt")
                        .about("Decrypt an encrypted vault file for viewing or editing")
                        .arg(
                            Arg::new("file")
                                .required(true)
                                .help("The encrypted vault file to decrypt"),
                        )
                        .arg(
                            Arg::new("output")
                                .short('o')
                                .long("output")
                                .takes_value(true)
                                .help("Specify the name of the decrypted output file"),
                        )
                        .arg(
                            Arg::new("temp")
                                .long("temp")
                                .takes_value(false)
                                .help("Decrypt to a temporary location without saving"),
                        ),
                )
                .subcommand(
                    Command::new("edit")
                        .about("Securely edit an encrypted vault file in your preferred editor")
                        .arg(
                            Arg::new("file")
                                .required(true)
                                .help("The encrypted vault file to edit"),
                        )
                        .arg(
                            Arg::new("editor")
                                .short('e')
                                .long("editor")
                                .takes_value(true)
                                .help("Specify the editor to use (e.g., vim, nano)"),
                        )
                        .arg(
                            Arg::new("backup").long("backup").takes_value(false).help(
                                "Create a backup of the original encrypted file before editing",
                            ),
                        ),
                )
                .subcommand(
                    Command::new("rotate")
                        .about("Change the password for an encrypted vault file")
                        .arg(
                            Arg::new("file")
                                .required(true)
                                .help("The encrypted vault file to update"),
                        )
                        .arg(
                            Arg::new("old-password")
                                .long("old-password")
                                .takes_value(true)
                                .help("The current password for the file"),
                        )
                        .arg(
                            Arg::new("new-password")
                                .long("new-password")
                                .takes_value(true)
                                .help("The new password for the file"),
                        ),
                )
                .subcommand(
                    Command::new("list")
                        .about("List all encrypted (.encrypted) files in the current directory")
                        .arg(
                            Arg::new("path")
                                .short('p')
                                .long("path")
                                .takes_value(true)
                                .help("Specify the directory to scan for encrypted files"),
                        ),
                )
                .subcommand(
                    Command::new("view")
                        .about("Securely view the decrypted contents of an encrypted vault file")
                        .arg(
                            Arg::new("file")
                                .required(true)
                                .help("The encrypted vault file to view"),
                        ),
                ),
        )
        .get_matches()
}
