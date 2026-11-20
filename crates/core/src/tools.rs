//! Running the system's own tools, read-only (DESIGN.md §7.3): dpkg, snap,
//! flatpak, journalctl and the language package managers. Behind a trait so
//! tests can answer with recorded output.
//!
//! Nothing here ever changes the system: callers pass only query commands
//! (`snap list`, `journalctl --disk-usage`, `dpkg-query -W` …). Commands that
//! would change something are shown to the user to copy, never run.

use std::process::{Command, Stdio};

/// What running a tool gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ran {
    /// The program is not installed (not on `PATH`).
    Missing,
    /// It ran but failed, or could not be started for another reason.
    Failed(String),
    /// Its standard output.
    Output(String),
}

impl Ran {
    pub fn output(self) -> Option<String> {
        match self {
            Ran::Output(text) => Some(text),
            _ => None,
        }
    }

    pub fn is_missing(&self) -> bool {
        matches!(self, Ran::Missing)
    }
}

pub trait Runner: Send + Sync {
    fn run(&self, program: &str, args: &[&str]) -> Ran;
}

/// Runs programs found on `PATH`, with `LC_ALL=C` so output parses the same
/// in every language.
#[derive(Debug, Default, Clone, Copy)]
pub struct System;

impl Runner for System {
    fn run(&self, program: &str, args: &[&str]) -> Ran {
        let result = Command::new(program)
            .args(args)
            .env("LC_ALL", "C.UTF-8")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .output();
        match result {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ran::Missing,
            Err(error) => Ran::Failed(error.to_string()),
            Ok(out) if !out.status.success() => {
                Ran::Failed(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
            Ok(out) => Ran::Output(String::from_utf8_lossy(&out.stdout).into_owned()),
        }
    }
}

/// Parses sizes as tools print them: "1.2G", "1.2 GB", "283.6 MB", "512kB",
/// "4096" (bytes). Decimal units (kB, MB, GB) are powers of 1000; single
/// letters and binary units (K, M, G, KiB, MiB, GiB) are powers of 1024.
pub fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim().replace('\u{a0}', " ");
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: f64 = number.replace(',', ".").parse().ok()?;
    let factor: f64 = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" | "bytes" => 1.0,
        "k" | "kib" => 1024.0,
        "kb" => 1000.0,
        "m" | "mib" => 1024f64.powi(2),
        "mb" => 1000f64.powi(2),
        "g" | "gib" => 1024f64.powi(3),
        "gb" => 1000f64.powi(3),
        "t" | "tib" => 1024f64.powi(4),
        "tb" => 1000f64.powi(4),
        _ => return None,
    };
    Some((number * factor).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_parse_in_both_unit_systems() {
        assert_eq!(parse_size("1.5G"), Some(1_610_612_736));
        assert_eq!(parse_size("1.5 GB"), Some(1_500_000_000));
        assert_eq!(parse_size("283.6\u{a0}MB"), Some(283_600_000));
        assert_eq!(parse_size("512kB"), Some(512_000));
        assert_eq!(parse_size("4096"), Some(4096));
        assert_eq!(parse_size("12 MiB"), Some(12 * 1024 * 1024));
        assert_eq!(parse_size("lots"), None);
    }

    #[test]
    fn a_missing_program_is_missing() {
        assert!(System.run("cairn-no-such-program", &[]).is_missing());
        assert_eq!(System.run("sh", &["-c", "echo hi"]), Ran::Output("hi\n".into()));
        assert!(matches!(System.run("sh", &["-c", "exit 3"]), Ran::Failed(_)));
    }
}
