//! The fixture-home guard (AGENTS.md rule 4).
//!
//! Development and test builds must never run against a real home folder,
//! because this program moves and trashes files. They refuse to start unless
//! the home folder contains a marker file written by the fixture generator.

use std::fmt;
use std::path::{Path, PathBuf};

/// Name of the marker file the fixture generator writes into the fake home.
pub const MARKER: &str = ".cairn-fixture";

/// Environment variables that switch the guard on in a release build.
pub const GUARD_ENV_VARS: [&str; 2] = ["CAIRN_DEV", "CAIRN_E2E"];

/// Whether the guard applies to this run.
///
/// It applies to every debug build, to builds with the `e2e` feature (passed in
/// by the caller, because features belong to the app crate), and to any build
/// started with `CAIRN_DEV` or `CAIRN_E2E` set to a non-empty value.
pub fn is_required(
    debug_build: bool,
    e2e_build: bool,
    env: impl Fn(&str) -> Option<String>,
) -> bool {
    debug_build
        || e2e_build
        || GUARD_ENV_VARS
            .iter()
            .any(|name| env(name).is_some_and(|value| !value.is_empty()))
}

/// Why the guard refused to let the program start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardError {
    /// No home folder could be determined.
    NoHome,
    /// The home folder exists but has no marker file.
    NotAFixture { home: PathBuf },
}

impl fmt::Display for GuardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GuardError::NoHome => write!(
                f,
                "refusing to start: no home folder is set. Development builds run only \
                 against a generated fixture home; use `make app`."
            ),
            GuardError::NotAFixture { home } => write!(
                f,
                "refusing to start: {} is not a fixture home (no {MARKER} file). \
                 Development builds never touch a real home folder; use `make app`, \
                 or `make fixture` to recreate the fake home.",
                home.display()
            ),
        }
    }
}

impl std::error::Error for GuardError {}

/// Checks that `home` is a fixture home.
///
/// Only reads; never creates anything.
pub fn check(home: Option<&Path>) -> Result<(), GuardError> {
    let home = home
        .filter(|h| !h.as_os_str().is_empty())
        .ok_or(GuardError::NoHome)?;
    if home.join(MARKER).is_file() {
        Ok(())
    } else {
        Err(GuardError::NotAFixture {
            home: home.to_path_buf(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cairn-guard-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn accepts_a_home_with_the_marker() {
        let home = temp_dir("ok");
        fs::write(home.join(MARKER), "").unwrap();
        assert_eq!(check(Some(&home)), Ok(()));
    }

    #[test]
    fn refuses_a_home_without_the_marker() {
        let home = temp_dir("missing");
        let err = check(Some(&home)).unwrap_err();
        assert_eq!(err, GuardError::NotAFixture { home: home.clone() });
        assert!(err.to_string().contains("not a fixture home"));
    }

    #[test]
    fn refuses_when_the_marker_is_a_directory() {
        let home = temp_dir("dir");
        fs::create_dir(home.join(MARKER)).unwrap();
        assert!(check(Some(&home)).is_err());
    }

    #[test]
    fn refuses_no_home_or_empty_home() {
        assert_eq!(check(None), Err(GuardError::NoHome));
        assert_eq!(check(Some(Path::new(""))), Err(GuardError::NoHome));
    }

    #[test]
    fn check_creates_nothing() {
        let home = temp_dir("readonly");
        check(Some(&home)).unwrap_err();
        assert_eq!(fs::read_dir(&home).unwrap().count(), 0);
    }

    #[test]
    fn required_in_debug_and_e2e_builds_or_with_env() {
        let no_env = |_: &str| None;
        assert!(is_required(true, false, no_env));
        assert!(is_required(false, true, no_env));
        assert!(!is_required(false, false, no_env));
        assert!(is_required(false, false, |k: &str| (k == "CAIRN_E2E")
            .then(|| "1".into())));
        assert!(!is_required(false, false, |k: &str| (k == "CAIRN_DEV").then(String::new)));
    }
}
