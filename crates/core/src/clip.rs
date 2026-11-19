//! Clipboard formats for files (P2-T04): `text/uri-list` (RFC 2483) and
//! `x-special/gnome-copied-files`, which Files (Nautilus) uses to tell a cut
//! from a copy. Pure functions; the app moves the bytes to the clipboard.

use std::path::{Path, PathBuf};

pub const URI_LIST: &str = "text/uri-list";
pub const GNOME_COPIED: &str = "x-special/gnome-copied-files";
pub const TEXT: &str = "text/plain;charset=utf-8";

/// What a paste would bring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payload {
    pub paths: Vec<PathBuf>,
    pub cut: bool,
}

fn uris(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|p| glib::filename_to_uri(p, None).ok())
        .map(|u| u.to_string())
        .collect()
}

/// The formats to offer, as (MIME type, contents).
pub fn encode(paths: &[PathBuf], cut: bool) -> Vec<(&'static str, String)> {
    let uris = uris(paths);
    let mut list = String::new();
    for uri in &uris {
        list.push_str(uri);
        list.push_str("\r\n");
    }
    let mut gnome = String::from(if cut { "cut" } else { "copy" });
    for uri in &uris {
        gnome.push('\n');
        gnome.push_str(uri);
    }
    let text = paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    vec![(GNOME_COPIED, gnome), (URI_LIST, list), (TEXT, text)]
}

fn local_paths<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<PathBuf> {
    lines
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|uri| glib::filename_from_uri(uri).ok().map(|(p, _)| p))
        .collect()
}

/// Reads `x-special/gnome-copied-files`: "cut" or "copy", then one URI per line.
pub fn decode_gnome(text: &str) -> Option<Payload> {
    let mut lines = text.lines();
    let cut = match lines.next()?.trim() {
        "cut" => true,
        "copy" => false,
        _ => return None,
    };
    let paths = local_paths(lines);
    (!paths.is_empty()).then_some(Payload { paths, cut })
}

/// Reads `text/uri-list` (always a copy).
pub fn decode_uri_list(text: &str) -> Option<Payload> {
    let paths = local_paths(text.lines());
    (!paths.is_empty()).then_some(Payload { paths, cut: false })
}

/// The paste payload from whatever the clipboard offers, preferring the
/// GNOME format (it knows about cut).
pub fn decode(gnome: &str, uri_list: &str) -> Option<Payload> {
    decode_gnome(gnome).or_else(|| decode_uri_list(uri_list))
}

pub fn is_local(path: &Path) -> bool {
    path.is_absolute()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_cut_and_copy() {
        let paths = vec![
            PathBuf::from("/home/sam/a b.txt"),
            PathBuf::from("/home/sam/ü.pdf"),
        ];
        for cut in [true, false] {
            let formats = encode(&paths, cut);
            let get = |m: &str| formats.iter().find(|(k, _)| *k == m).unwrap().1.clone();
            assert_eq!(
                decode(&get(GNOME_COPIED), ""),
                Some(Payload {
                    paths: paths.clone(),
                    cut
                })
            );
            assert_eq!(
                decode("", &get(URI_LIST)),
                Some(Payload {
                    paths: paths.clone(),
                    cut: false
                })
            );
        }
    }

    #[test]
    fn encodes_like_files_does() {
        let formats = encode(&[PathBuf::from("/tmp/a b")], true);
        assert_eq!(
            formats[0],
            (GNOME_COPIED, "cut\nfile:///tmp/a%20b".to_string())
        );
        assert_eq!(formats[1], (URI_LIST, "file:///tmp/a%20b\r\n".to_string()));
    }

    #[test]
    fn ignores_comments_other_schemes_and_junk() {
        assert_eq!(
            decode_uri_list("# comment\r\nhttps://example.com/x\r\n"),
            None
        );
        assert_eq!(decode_gnome("move\nfile:///tmp/a"), None);
        assert_eq!(decode("", ""), None);
        let p = decode_uri_list("file:///tmp/a\nfile:///tmp/b\n").unwrap();
        assert_eq!(p.paths.len(), 2);
    }
}
