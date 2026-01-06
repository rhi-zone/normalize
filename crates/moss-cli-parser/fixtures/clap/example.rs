//! Example clap CLI for testing help output parsing.
//!
//! Build and run: cargo run -- --help
//! Or for subcommand: cargo run -- build --help

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "example")]
#[command(version = "1.0.0")]
#[command(about = "An example CLI tool for testing")]
struct Cli {
    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Config file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    /// Port number
    #[arg(short, long, default_value = "8080")]
    port: u16,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the project
    Build {
        /// Build in release mode
        #[arg(short, long)]
        release: bool,

        /// Target directory
        #[arg(short, long, value_name = "DIR")]
        target: Option<String>,
    },

    /// Run the project
    Run {
        /// Arguments to pass
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Clean build artifacts
    Clean,
}

fn main() {
    let _cli = Cli::parse();
    println!("CLI parsed successfully");
}
