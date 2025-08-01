use clap::{Args, Parser, Subcommand};

/// A CLI application for managing and executing HTTP requests or collections.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// The primary command to execute.
    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level commands for the CLI.
#[derive(Subcommand)]
pub enum Commands {
    /// Work with request collections (grouped sets of HTTP requests).
    Collections {
        /// Subcommands related to request collections.
        #[command(subcommand)]
        command: CollectionCommands,
    },
    /// Work with individual HTTP requests.
    Requests {
        /// Subcommands related to individual HTTP requests.
        #[command(subcommand)]
        command: RequestCommands,
    },
}

/// Subcommands for managing and executing request collections.
#[derive(Subcommand)]
pub enum CollectionCommands {
    /// Execute an entire collection of requests.
    Run {
        /// The collection file containing the requests to execute.
        collection: String,

        /// Additional options for controlling output behavior.
        #[command(flatten)]
        output_options: OutputOptions,
    },
}

/// Additional output options for fine-tuning the CLI's behavior.
#[derive(Args, Clone)]
pub struct OutputOptions {
    /// Displays the HTTP headers in the output (disabled by default).
    #[arg(short = 'h', long, default_value_t = false)]
    pub show_headers: bool,

    /// Suppresses the HTTP response status (enabled by default).
    #[arg(short = 's', long, default_value_t = false)]
    pub hide_status: bool,

    /// Suppresses the HTTP response body (enabled by default).
    #[arg(short = 'b', long, default_value_t = false)]
    pub hide_body: bool,

    /// Disables pretty-printing for the HTTP response (enabled by default).
    #[arg(short = 'r', long, default_value_t = false)]
    pub raw_output: bool,

    /// Disables pre-output masking (enabled by default).
    #[arg(short = 'm', long, default_value_t = false)]
    pub disable_masking: bool,
}

/// Subcommands for managing and executing individual requests.
#[derive(Subcommand)]
pub enum RequestCommands {
    /// Execute a specific request within a collection.
    Run {
        /// The collection file containing the requests.
        collection: String,

        /// The specific request to execute within the collection.
        request: String,

        /// Additional options for controlling output behavior.
        #[command(flatten)]
        output_options: OutputOptions,
    },
}
