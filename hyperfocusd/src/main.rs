use clap::{Parser, Subcommand};
use listenfd::ListenFd;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
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
        "hyperfocusd" | _ => {
            // When called as hyperfocusd or anything else, parse subcommands
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
    }
}

fn execute_hook(hook: &Hook) {
    if hook.argv.is_empty() {
        eprintln!("Warning: hook has empty argv");
        return;
    }

    let result = Command::new(&hook.argv[0])
        .args(&hook.argv[1..])
        .status();

    match result {
        Ok(status) => {
            if !status.success() {
                eprintln!("Hook {:?} failed with status: {}", hook.argv, status);
            }
        }
        Err(e) => {
            eprintln!("Failed to execute hook {:?}: {}", hook.argv, e);
        }
    }
}

fn run_daemon(config_path: Option<PathBuf>) {
    // Load configuration if provided
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

    // Get the socket from systemd or listen on our own
    let mut listenfd = ListenFd::from_env();
    let listener = listenfd
        .take_unix_listener(0)
        .expect("Failed to get socket from systemd")
        .expect("No socket provided by systemd");

    // Notify systemd that we're ready
    let _ = sd_notify::notify(true, &[sd_notify::NotifyState::Ready]);

    eprintln!("hyperfocusd daemon started and ready");

    // Accept connections and handle them sequentially
    // The single-threaded loop provides mutual exclusion naturally
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                eprintln!("Client connected");

                // Read the request from the client using BufReader
                let mut reader = BufReader::new(stream);
                let mut line = String::new();

                if reader.read_line(&mut line).is_ok() {
                    eprintln!("Received request: {}", line.trim());

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
                            eprintln!("Client disconnected without sending DONE (crashed or killed?)");
                        }
                        Ok(_) => {
                            if line.trim() == "DONE" {
                                eprintln!("Client finished cleanly");
                            } else {
                                eprintln!("Client sent unexpected message: {}", line.trim());
                            }
                        }
                        Err(e) => {
                            eprintln!("Error reading from client: {}", e);
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
                eprintln!("Connection error: {}", e);
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
