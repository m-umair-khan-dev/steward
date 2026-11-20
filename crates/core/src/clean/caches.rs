//! The developer-cache allow-list (`data/clean/dev-caches.toml`).
//!
//! The file uses a small subset of TOML (`[[cache]]` tables of string values)
//! and is read with a parser for exactly that subset, so the engine needs no
//! TOML library. Anything outside the subset is a build-time test failure.

use std::path::PathBuf;

const ALLOW_LIST: &str = include_str!("../../../../data/clean/dev-caches.toml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevCache {
    pub id: String,
    pub name: String,
    /// Relative to the home folder.
    pub path: PathBuf,
    pub reason: String,
}

/// Parses the allow-list format. Errors name the line.
pub(crate) fn parse(text: &str) -> Result<Vec<DevCache>, String> {
    let mut out = Vec::new();
    let mut current: Option<Vec<(String, String)>> = None;
    let finish = |fields: Vec<(String, String)>, out: &mut Vec<DevCache>| -> Result<(), String> {
        let get = |key: &str| {
            fields
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .ok_or(format!("a cache has no {key}"))
        };
        out.push(DevCache {
            id: get("id")?,
            name: get("name")?,
            path: PathBuf::from(get("path")?),
            reason: get("reason")?,
        });
        Ok(())
    };
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[cache]]" {
            if let Some(fields) = current.take() {
                finish(fields, &mut out)?;
            }
            current = Some(Vec::new());
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or(format!("line {}: expected key = \"value\"", number + 1))?;
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .filter(|v| !v.contains('"') && !v.contains('\\'))
            .ok_or(format!("line {}: expected a plain string", number + 1))?;
        current
            .as_mut()
            .ok_or(format!("line {}: outside a [[cache]] table", number + 1))?
            .push((key.trim().to_string(), value.to_string()));
    }
    if let Some(fields) = current.take() {
        finish(fields, &mut out)?;
    }
    Ok(out)
}

/// The allow-list built into the program.
pub fn dev_caches() -> Vec<DevCache> {
    parse(ALLOW_LIST).expect("data/clean/dev-caches.toml is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_allow_list_parses() {
        let caches = dev_caches();
        assert!(caches.iter().any(|c| c.path == PathBuf::from(".npm/_cacache")));
        assert!(caches.iter().all(|c| c.path.is_relative()));
        let mut ids: Vec<_> = caches.iter().map(|c| c.id.clone()).collect();
        ids.dedup();
        assert_eq!(ids.len(), caches.len(), "ids are unique");
    }

    #[test]
    fn anything_outside_the_subset_is_refused() {
        assert!(parse("id = \"x\"").is_err());
        assert!(parse("[[cache]]\nid = x").is_err());
        assert!(parse("[[cache]]\nid = \"x\"").is_err(), "missing fields");
    }
}
