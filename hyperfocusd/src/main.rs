use clap::{Parser, Subcommand};
use std::env;
use std::process;

#[derive(Parser)]
#[command(name = "hyperfocusd")]
#[command(about = "Benchmark environment switch daemon", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the hyperfocusd daemon
    Daemon,
    /// Run a command in hyperfocus mode
    On {
        /// Command to run
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
}

fn main() {
    // Multi-call binary: determine which command to run based on argv[0]
    let program_name = env::args()
        .next()
        .and_then(|path| {
            std::path::Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(String::from)
        })
        .unwrap_or_else(|| String::from("hyperfocusd"));

    match program_name.as_str() {
        "hyperfocus-on" => {
            // When called as hyperfocus-on, run the on command
            run_on_command();
        }
        "hyperfocusd" | _ => {
            // When called as hyperfocusd or anything else, parse subcommands
            let cli = Cli::parse();

            match cli.command {
                Some(Commands::Daemon) => {
                    run_daemon();
                }
                Some(Commands::On { command }) => {
                    run_on_with_args(command);
                }
                None => {
                    // Default: run daemon
                    run_daemon();
                }
            }
        }
    }
}

fn run_daemon() {
    eprintln!("hyperfocusd daemon not yet implemented");
    process::exit(1);
}

fn run_on_command() {
    // Parse args after "hyperfocus-on" as the command to run
    let args: Vec<String> = env::args().skip(1).collect();

    // Skip the "--" separator if present
    let command_args: Vec<String> = if args.first().map(|s| s.as_str()) == Some("--") {
        args.into_iter().skip(1).collect()
    } else {
        args
    };

    run_on_with_args(command_args);
}

fn run_on_with_args(command: Vec<String>) {
    if command.is_empty() {
        eprintln!("Error: no command specified");
        eprintln!("Usage: hyperfocus-on -- <command> [args...]");
        process::exit(1);
    }

    eprintln!("hyperfocus-on not yet implemented");
    eprintln!("Would run: {:?}", command);
    process::exit(1);
}
