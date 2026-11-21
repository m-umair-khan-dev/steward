//! Markdown (P7-T07; DESIGN.md §4.2, §8.5): the outline of a document, and
//! the text the preview renders. The preview has no scripts (QML's own
//! Markdown, no web engine) and no network: remote images are removed before
//! rendering, and local images are allowed only from the document's folder.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: usize,
    pub text: String,
    /// 0-based line in the document.
    pub line: usize,
}

/// ATX headings ("# Title"), not counting lines inside code fences.
pub fn outline(text: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut fence: Option<&str> = None;
    for (line_no, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        for marker in ["```", "~~~"] {
            if trimmed.starts_with(marker) {
                fence = match fence {
                    Some(open) if open == marker => None,
                    None => Some(marker),
                    other => other,
                };
            }
        }
        if fence.is_some() || trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            continue;
        }
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&level) && trimmed[level..].starts_with(' ') {
            let text = trimmed[level..]
                .trim()
                .trim_end_matches('#')
                .trim()
                .to_string();
            if !text.is_empty() {
                out.push(Heading {
                    level,
                    text,
                    line: line_no,
                });
            }
        }
    }
    out
}

/// True if an image address is a local file inside `folder` (relative, or
/// file:// under it). Everything else (http, https, data, other folders)
/// is not loaded.
fn local_image(address: &str, folder: &Path) -> Option<String> {
    let address = address.trim().trim_start_matches('<').trim_end_matches('>');
    if address.contains("://") && !address.starts_with("file://") {
        return None;
    }
    if address.starts_with("data:") || address.starts_with("//") {
        return None;
    }
    let path = address.strip_prefix("file://").unwrap_or(address);
    let full = if Path::new(path).is_absolute() {
        Path::new(path).to_path_buf()
    } else {
        folder.join(path)
    };
    // Only inside the document's folder; ".." may not climb out.
    let full = full.canonicalize().ok()?;
    let folder = folder.canonicalize().ok()?;
    full.starts_with(&folder)
        .then(|| format!("file://{}", full.display()))
}

/// The document as the preview renders it: images become local file URLs
/// when they are inside the document's folder, and alt text otherwise.
/// HTML is left to QML's renderer, which runs no scripts and loads nothing
/// for tags it does not support.
pub fn preview(text: &str, folder: &Path) -> String {
    let text = &inline_reference_images(text);
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(start) = rest.find("![") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let parsed = after.find("](").and_then(|close| {
            let alt = &after[..close];
            let tail = &after[close + 2..];
            let end = tail.find(')')?;
            let target = tail[..end].split_whitespace().next().unwrap_or("");
            Some((alt, target, close + 2 + end + 1))
        });
        match parsed {
            Some((alt, target, used)) => {
                match local_image(target, folder) {
                    Some(url) => out.push_str(&format!("![{alt}]({url})")),
                    None => {
                        out.push_str(&format!("[{}]", if alt.is_empty() { "image" } else { alt }))
                    }
                }
                rest = &after[used..];
            }
            None => {
                out.push_str("![");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    // HTML images (<img src=…>) are dropped the same way: only local ones stay.
    strip_remote_img_tags(&out, folder)
}

/// Reference definitions: `[label]: address` lines, by lower-case label.
fn definitions(text: &str) -> std::collections::HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let rest = line.strip_prefix('[')?;
            let (label, after) = rest.split_once("]:")?;
            let address = after.split_whitespace().next()?;
            Some((label.trim().to_lowercase(), address.to_string()))
        })
        .collect()
}

/// `![alt][label]`, `![alt][]` and `![label]` become `![alt](address)`, so
/// one place decides which images may load. An image whose label has no
/// definition is left as text.
fn inline_reference_images(text: &str) -> String {
    let defs = definitions(text);
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("![") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(close) = after.find(']') else {
            out.push_str("![");
            rest = after;
            continue;
        };
        let alt = &after[..close];
        let tail = &after[close + 1..];
        if tail.starts_with('(') {
            out.push_str("![");
            rest = after;
            continue;
        }
        let (label, used) = match tail
            .strip_prefix('[')
            .and_then(|t| t.find(']').map(|e| (&t[..e], e + 2)))
        {
            Some(("", used)) => (alt, used),
            Some((label, used)) => (label, used),
            None => (alt, 0),
        };
        match defs.get(&label.trim().to_lowercase()) {
            Some(address) => {
                out.push_str(&format!("![{alt}]({address})"));
                rest = &tail[used..];
            }
            None => {
                out.push_str("![");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn strip_remote_img_tags(text: &str, folder: &Path) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.to_ascii_lowercase().find("<img") {
        out.push_str(&rest[..start]);
        let tag_end = rest[start..]
            .find('>')
            .map(|e| start + e + 1)
            .unwrap_or(rest.len());
        let tag = &rest[start..tag_end];
        let src = tag
            .split("src=")
            .nth(1)
            .and_then(|v| {
                let q = v.chars().next()?;
                if q == '"' || q == '\'' {
                    v[1..].split(q).next()
                } else {
                    v.split([' ', '>']).next()
                }
            })
            .unwrap_or("");
        if let Some(url) = local_image(src, folder) {
            out.push_str(&format!("<img src=\"{url}\">"));
        }
        rest = &rest[tag_end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_skips_code() {
        let text =
            "# Title\n\nText\n\n## Part one ##\n```\n# not a heading\n```\n### Deep\n#nospace\n";
        let o = outline(text);
        assert_eq!(
            o.iter()
                .map(|h| (h.level, h.text.as_str(), h.line))
                .collect::<Vec<_>>(),
            [(1, "Title", 0), (2, "Part one", 4), (3, "Deep", 8)]
        );
    }

    #[test]
    fn remote_images_are_removed_and_local_ones_kept() {
        let dir = std::env::temp_dir().join(format!("cairn-md-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pic.png"), b"x").unwrap();
        let text = "A ![logo](https://example.invalid/logo.png) and ![](pic.png \"t\") and ![up](../etc/passwd)\n<img src=\"http://x.invalid/a.png\"> <IMG src='pic.png'>";
        let out = preview(text, &dir);
        assert!(
            !out.contains("example.invalid") && !out.contains("x.invalid"),
            "{out}"
        );
        assert!(out.contains("[logo]") && out.contains("[up]"));
        assert!(out.contains(&format!(
            "file://{}",
            dir.join("pic.png").canonicalize().unwrap().display()
        )));
        let refs = "![a][r1] ![r2] ![b][]\n\n[r1]: https://one.invalid/a.png\n[R2]: http://two.invalid/b.png\n[b]: pic.png\n";
        let out = preview(refs, &dir);
        assert!(!out.contains("![a]") && !out.contains("![r2]"), "{out}");
        assert!(out.contains("![b](file://"), "{out}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
