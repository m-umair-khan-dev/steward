//! `steward-cli`: the `steward` commands without loading Qt (P7-T10; DESIGN.md
//! §4.2). A running Steward gets the request over D-Bus; otherwise `steward`
//! itself is started with the same arguments. See `steward_core::cli`.

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use steward_core::cli::{self, Parsed};

/// The app: next to this program (development, tests), else on PATH.
fn app_binary() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|me| me.parent().map(|dir| dir.join("steward")))
        .filter(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from("steward"))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let requests = match cli::parse(&args, &cli::absolute) {
        Parsed::Run(requests) => requests,
        Parsed::Help => {
            println!("{}", cli::USAGE);
            return ExitCode::SUCCESS;
        }
        Parsed::Version => {
            println!("steward-cli {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Parsed::Wrong(message) => {
            eprintln!("steward-cli: {message}\n\n{}", cli::USAGE);
            return ExitCode::from(2);
        }
    };
    if cli::forward(&requests, "steward-cli") {
        return ExitCode::SUCCESS;
    }
    // No Steward running: become it.
    let error = Command::new(app_binary()).args(&args).exec();
    eprintln!("steward-cli: could not start steward: {error}");
    ExitCode::FAILURE
}
