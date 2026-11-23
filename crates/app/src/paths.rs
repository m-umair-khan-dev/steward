//! Path helpers for the path bar (P1-T05): `~` display and expansion, Tab
//! completion of folder names, and breadcrumb segments.

use std::path::{Path, PathBuf};

/// `~/Documents` for folders under home, the full path otherwise.
pub fn display_path(path: &Path) -> String {
    let home = glib::home_dir();
    match path.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Expands a leading `~` and makes relative paths relative to `base`.
pub fn expand(text: &str, base: Option<&Path>) -> PathBuf {
    let text = text.trim();
    let home = glib::home_dir();
    if text == "~" {
        return home;
    }
    if let Some(rest) = text.strip_prefix("~/") {
        return home.join(rest);
    }
    let path = PathBuf::from(text);
    if path.is_absolute() {
        path
    } else {
        base.map(|b| b.join(&path)).unwrap_or(path)
    }
}

/// Completes the last part of `text` to a folder name inside its parent.
/// One match: the full name plus "/". Several: their longest common prefix.
pub fn complete(text: &str, base: Option<&Path>) -> Option<String> {
    let (dir_part, prefix) = match text.rfind('/') {
        Some(i) => (&text[..=i], &text[i + 1..]),
        None if text == "~" => return Some("~/".into()),
        None => ("", text),
    };
    let dir = if dir_part.is_empty() {
        base?.to_path_buf()
    } else {
        expand(dir_part, base)
    };
    let folders: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let visible = |name: &&String| prefix.starts_with('.') || !name.starts_with('.');
    let mut matches: Vec<String> = folders
        .iter()
        .filter(visible)
        .filter(|n| n.starts_with(prefix))
        .cloned()
        .collect();
    if matches.is_empty() {
        let lower = prefix.to_lowercase();
        matches = folders
            .iter()
            .filter(visible)
            .filter(|n| n.to_lowercase().starts_with(&lower))
            .cloned()
            .collect();
    }
    matches.sort();
    match matches.as_slice() {
        [] => None,
        [one] => Some(format!("{dir_part}{one}/")),
        many => {
            let first = &many[0];
            let common = first
                .char_indices()
                .map(|(i, c)| i + c.len_utf8())
                .take_while(|&end| many.iter().all(|m| m.starts_with(&first[..end])))
                .last()
                .unwrap_or(0);
            (common > prefix.len()).then(|| format!("{dir_part}{}", &first[..common]))
        }
    }
}

/// Breadcrumbs from the disk root: ("Computer", "/"), ("home", "/home"), ...
pub fn segments(path: &Path, root_label: &str) -> Vec<(String, PathBuf)> {
    let mut current = PathBuf::from("/");
    let mut out = vec![(root_label.to_string(), current.clone())];
    if let Ok(rest) = path.strip_prefix("/") {
        for part in rest.components() {
            current.push(part);
            out.push((
                part.as_os_str().to_string_lossy().into_owned(),
                current.clone(),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_tree() -> PathBuf {
        let root = std::env::temp_dir().join(format!("steward-paths-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["Documents", "Downloads", "Desktop", "Music", ".config"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("Docfile.txt"), "").unwrap();
        root
    }

    #[test]
    fn completes_a_unique_folder() {
        let root = temp_tree();
        let text = format!("{}/Doc", root.display());
        assert_eq!(
            complete(&text, None),
            Some(format!("{}/Documents/", root.display()))
        );
    }

    #[test]
    fn completes_only_when_it_adds_something() {
        let root = temp_tree();
        assert_eq!(complete(&format!("{}/D", root.display()), None), None);
        assert_eq!(complete(&format!("{}/Do", root.display()), None), None);
        assert_eq!(
            complete(&format!("{}/Dow", root.display()), None),
            Some(format!("{}/Downloads/", root.display()))
        );
    }

    #[test]
    fn relative_and_case_insensitive() {
        let root = temp_tree();
        assert_eq!(complete("Mu", Some(&root)), Some("Music/".into()));
        assert_eq!(complete("mu", Some(&root)), Some("Music/".into()));
        assert_eq!(complete(".co", Some(&root)), Some(".config/".into()));
        assert_eq!(complete("", Some(&root)), None);
    }

    #[test]
    fn expand_handles_home_and_relative_paths() {
        let home = glib::home_dir();
        assert_eq!(expand("~", None), home);
        assert_eq!(expand("~/Documents", None), home.join("Documents"));
        assert_eq!(expand("/etc", None), PathBuf::from("/etc"));
        assert_eq!(
            expand("x", Some(Path::new("/tmp"))),
            PathBuf::from("/tmp/x")
        );
    }

    #[test]
    fn segments_start_at_the_root() {
        let s = segments(Path::new("/home/sam/Downloads"), "Computer");
        let labels: Vec<&str> = s.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["Computer", "home", "sam", "Downloads"]);
        assert_eq!(s[3].1, PathBuf::from("/home/sam/Downloads"));
    }
}
