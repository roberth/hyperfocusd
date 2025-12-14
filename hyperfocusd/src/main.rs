use clap::{Parser, Subcommand};
use listenfd::ListenFd;
use log::{debug, error, info, warn, LevelFilter};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{self, Command, Stdio};

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
    Daemon {
        /// Path to configuration file
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Run a command in hyperfocus mode
    On {
        /// Command to run
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct Config {
    #[serde(default)]
    hooks: Hooks,
    #[serde(default = "default_log_level")]
    log_level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn journal_logger_error_exit() -> ! {
    eprintln!("Note: The daemon currently requires systemd with journal logging.");
    eprintln!("Standalone usage is not supported yet, but could easily be added as a feature.");
    process::exit(1);
}

fn parse_log_level(level: &str) -> LevelFilter {
    match level.to_lowercase().as_str() {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        // "trace" => LevelFilter::Trace,
        "trace" => {
            eprintln!("Log level 'trace' is not supported. Use: off, error, warn, info, debug");
            process::exit(1);
        }
        _ => {
            eprintln!("Invalid log level '{}'. Valid values: off, error, warn, info, debug", level);
            process::exit(1);
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct Hooks {
    #[serde(rename = "startFocus", default)]
    start_focus: Option<Hook>,
    #[serde(rename = "stopFocus", default)]
    stop_focus: Option<Hook>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Hook {
    argv: Vec<String>,
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
        "hyperfocusd" => {
            // When called as hyperfocusd, parse subcommands
            let cli = Cli::parse();

            match cli.command {
                Some(Commands::Daemon { config }) => {
                    run_daemon(config);
                }
                Some(Commands::On { command }) => {
                    run_on_with_args(command);
                }
                None => {
                    // Default: run daemon
                    run_daemon(None);
                }
            }
        }
        unknown => {
            eprintln!("Unknown command name: {}", unknown);
            eprintln!("This binary should be called as 'hyperfocusd' or 'hyperfocus-on'.");
            process::exit(1);
        }
    }
}

fn execute_hook(hook: &Hook) {
    if hook.argv.is_empty() {
        warn!("Hook has empty argv");
        return;
    }

    debug!("Executing hook: {:?}", hook.argv);

    let result = Command::new(&hook.argv[0])
        .args(&hook.argv[1..])
        .status();

    match result {
        Ok(status) => {
            if status.success() {
                info!("Hook completed successfully");
            } else {
                error!("Hook {:?} failed with status: {}", hook.argv, status);
            }
        }
        Err(e) => {
            error!("Failed to execute hook {:?}: {}", hook.argv, e);
        }
    }
}

fn run_daemon(config_path: Option<PathBuf>) {
    // Load configuration first to get log level
    let config = config_path.map(|path| {
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|e| {
                eprintln!("Failed to read config file {:?}: {}", path, e);
                process::exit(1);
            });
        serde_json::from_str::<Config>(&contents)
            .unwrap_or_else(|e| {
                eprintln!("Failed to parse config file {:?}: {}", path, e);
                process::exit(1);
            })
    });

    // Initialize systemd journal logger
    let journal_log = match systemd_journal_logger::JournalLog::new() {
        Ok(logger) => logger,
        Err(e) => {
            eprintln!("Failed to create systemd journal logger: {}", e);
            journal_logger_error_exit();
        }
    };

    if let Err(e) = journal_log.install() {
        eprintln!("Failed to install systemd journal logger: {}", e);
        journal_logger_error_exit();
    }

    // Set log level from config
    let level = config.as_ref()
        .map(|c| parse_log_level(&c.log_level))
        .unwrap_or(LevelFilter::Info);
    log::set_max_level(level);

    // Get the socket from systemd or listen on our own
    let mut listenfd = ListenFd::from_env();
    let listener = listenfd
        .take_unix_listener(0)
        .expect("Failed to get socket from systemd")
        .expect("No socket provided by systemd");

    // Notify systemd that we're ready
    let _ = sd_notify::notify(true, &[sd_notify::NotifyState::Ready]);

    info!("hyperfocusd daemon started and ready");

    // Accept connections and handle them sequentially
    // The single-threaded loop provides mutual exclusion naturally
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                debug!("Client connected");

                // Read the request from the client using BufReader
                let mut reader = BufReader::new(stream);
                let mut line = String::new();

                if reader.read_line(&mut line).is_ok() {
                    debug!("Received request: {}", line.trim());

                    // Execute startFocus hook if configured
                    if let Some(ref cfg) = config {
                        if let Some(ref hook) = cfg.hooks.start_focus {
                            execute_hook(hook);
                        }
                    }

                    // Acknowledge
                    let _ = reader.get_mut().write_all(b"OK\n");

                    // Wait for client to finish and send DONE message
                    // This blocks the loop, ensuring only one client is active at a time
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => {
                            warn!("Client disconnected without sending DONE (crashed or killed?)");
                        }
                        Ok(_) => {
                            if line.trim() == "DONE" {
                                debug!("Client finished cleanly");
                            } else {
                                warn!("Client sent unexpected message: {}", line.trim());
                            }
                        }
                        Err(e) => {
                            error!("Error reading from client: {}", e);
                        }
                    }

                    // Execute stopFocus hook if configured
                    if let Some(ref cfg) = config {
                        if let Some(ref hook) = cfg.hooks.stop_focus {
                            execute_hook(hook);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Connection error: {}", e);
            }
        }
    }
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

    // Connect to the daemon socket
    let socket_path = "/run/hyperfocusd/hyperfocusd.socket";
    let mut stream = UnixStream::connect(socket_path)
        .unwrap_or_else(|e| {
            eprintln!("Failed to connect to hyperfocusd: {}", e);
            process::exit(1);
        });

    // Send request to enter hyperfocus mode
    stream.write_all(b"START\n").unwrap();

    // Wait for acknowledgment
    {
        let reader = BufReader::new(&stream);
        let mut lines = reader.lines();
        if let Some(Ok(response)) = lines.next() {
            if response != "OK" {
                eprintln!("Unexpected response from daemon: {}", response);
                process::exit(1);
            }
        }
    } // reader dropped here, but stream remains alive
    // Run the command with HYPERFOCUSING=1 environment variable
    // The stream stays open during command execution, ensuring mutual exclusion
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .env("HYPERFOCUSING", "1")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap_or_else(|e| {
            eprintln!("Failed to execute command: {}", e);
            process::exit(1);
        });

    // Wait for the command to complete
    let status = child.wait().unwrap();

    // Send goodbye message to daemon
    let _ = stream.write_all(b"DONE\n");

    // Close connection to daemon
    drop(stream);

    // Exit with the same status code as the child process
    process::exit(status.code().unwrap_or(1));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log_level_valid() {
        assert_eq!(parse_log_level("off"), LevelFilter::Off);
        assert_eq!(parse_log_level("error"), LevelFilter::Error);
        assert_eq!(parse_log_level("warn"), LevelFilter::Warn);
        assert_eq!(parse_log_level("info"), LevelFilter::Info);
        assert_eq!(parse_log_level("debug"), LevelFilter::Debug);
    }

    #[test]
    fn test_parse_log_level_case_insensitive() {
        assert_eq!(parse_log_level("INFO"), LevelFilter::Info);
        assert_eq!(parse_log_level("Debug"), LevelFilter::Debug);
    }

    // Note: test_parse_log_level_invalid and test_parse_log_level_rejects_trace
    // cannot be unit tested because they call process::exit(1).
    // They are tested indirectly through the NixOS VM test which validates
    // the enum at the Nix level.
    //
    // If you add trace support, remember to:
    // 1. Remove the explicit "trace" match arm that calls process::exit(1)
    // 2. Uncomment the "trace" => LevelFilter::Trace line
    // 3. Add "trace" to the NixOS module log_level enum in flake/nixos-module.nix
    // 4. Update the "Invalid log level" error message to include "trace"
}
