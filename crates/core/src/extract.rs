//! Text from documents, for search inside files (P7-T09; DESIGN.md §7.1
//! `extract`, §8.5). File content is untrusted, so extraction runs in a
//! `bwrap` sandbox: no network, no home, a read-only `/usr`, and a
//! read-only view of the one input file, with a time and size limit
//! (ADR 0005 profile).
//!
//! Two halves:
//! - [`text_of`] runs **inside** the sandbox (the indexer started as
//!   `steward-indexer extract FILE`): plain text and code, Markdown, PDF
//!   (`pdftotext`), OpenDocument and Office files (zipped XML).
//! - [`Sandbox::extract`] runs outside: it starts the sandbox and reads
//!   its output, and stops it when the time is up.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Text kept per file.
pub const TEXT_LIMIT: usize = 1024 * 1024;
/// Files larger than this are not read.
pub const SIZE_LIMIT: u64 = 25 * 1024 * 1024;

const TEXT_EXTENSIONS: &[&str] = &[
    "txt",
    "text",
    "md",
    "markdown",
    "rst",
    "org",
    "adoc",
    "tex",
    "csv",
    "tsv",
    "json",
    "yaml",
    "yml",
    "toml",
    "ini",
    "conf",
    "cfg",
    "log",
    "xml",
    "html",
    "htm",
    "css",
    "scss",
    "js",
    "mjs",
    "ts",
    "jsx",
    "tsx",
    "vue",
    "svelte",
    "py",
    "rs",
    "go",
    "c",
    "h",
    "cc",
    "cpp",
    "hpp",
    "cs",
    "java",
    "kt",
    "swift",
    "rb",
    "php",
    "pl",
    "lua",
    "sh",
    "bash",
    "zsh",
    "fish",
    "sql",
    "r",
    "dart",
    "scala",
    "ex",
    "exs",
    "erl",
    "hs",
    "ml",
    "nim",
    "zig",
    "gradle",
    "properties",
    "env",
];

/// What kind of extraction a file needs, by its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Pdf,
    OpenDocument,
    Docx,
    Xlsx,
    Pptx,
}

pub fn format_of(path: &Path) -> Option<Format> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "pdf" => Format::Pdf,
        "odt" | "ods" | "odp" | "odg" => Format::OpenDocument,
        "docx" | "docm" => Format::Docx,
        "xlsx" | "xlsm" => Format::Xlsx,
        "pptx" | "pptm" => Format::Pptx,
        e if TEXT_EXTENSIONS.contains(&e) => Format::Text,
        _ => return None,
    })
}

// --- inside the sandbox ---------------------------------------------------------------------------

fn truncate(mut text: String) -> String {
    if text.len() > TEXT_LIMIT {
        let mut cut = TEXT_LIMIT;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
    }
    text
}

/// The text of XML, with a line break after each paragraph-like element.
pub fn xml_text(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 3);
    let mut rest = xml;
    while let Some(open) = rest.find('<') {
        out.push_str(&decode(&rest[..open]));
        let Some(close) = rest[open..].find('>') else {
            break;
        };
        let tag = &rest[open + 1..open + close];
        let name = tag
            .trim_start_matches('/')
            .split([' ', '/', '\t', '\n'])
            .next()
            .unwrap_or("");
        let ends_block = tag.starts_with('/')
            && matches!(
                name,
                "text:p" | "text:h" | "w:p" | "a:p" | "si" | "table:table-row" | "p"
            );
        if ends_block || matches!(name, "text:line-break" | "w:br" | "text:tab" | "w:tab") {
            out.push(if name.ends_with("tab") { '\t' } else { '\n' });
        }
        rest = &rest[open + close + 1..];
    }
    out
}

fn decode(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn zip_parts(path: &Path, wanted: impl Fn(&str) -> bool) -> std::io::Result<String> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(std::io::Error::other)?;
    let mut names: Vec<String> = archive
        .file_names()
        .filter(|n| wanted(n))
        .map(str::to_string)
        .collect();
    // Slides and sheets in their natural order (slide2 before slide10).
    names.sort_by(|a, b| crate::natural::compare(a, b));
    let mut out = String::new();
    for name in names {
        let mut part = archive.by_name(&name).map_err(std::io::Error::other)?;
        let mut xml = String::new();
        part.by_ref()
            .take(TEXT_LIMIT as u64 * 8)
            .read_to_string(&mut xml)?;
        out.push_str(&xml_text(&xml));
        out.push('\n');
        if out.len() > TEXT_LIMIT {
            break;
        }
    }
    Ok(out)
}

/// The text of one file. Runs inside the sandbox.
pub fn text_of(path: &Path) -> std::io::Result<String> {
    let format = format_of(path).ok_or_else(|| std::io::Error::other("not a supported format"))?;
    let text = match format {
        Format::Text => {
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(TEXT_LIMIT as u64)
                .read_to_end(&mut bytes)?;
            if bytes.iter().take(8192).any(|b| *b == 0) {
                return Err(std::io::Error::other("binary"));
            }
            String::from_utf8_lossy(&bytes).into_owned()
        }
        Format::Pdf => {
            let output = Command::new("pdftotext")
                .args(["-q", "-enc", "UTF-8"])
                .arg(path)
                .arg("-")
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()?;
            if !output.status.success() {
                return Err(std::io::Error::other("pdftotext failed"));
            }
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        Format::OpenDocument => zip_parts(path, |n| n == "content.xml")?,
        Format::Docx => zip_parts(path, |n| {
            n == "word/document.xml"
                || (n.starts_with("word/") && (n.contains("/header") || n.contains("/footer")))
        })?,
        Format::Xlsx => zip_parts(path, |n| n == "xl/sharedStrings.xml")?,
        Format::Pptx => zip_parts(path, |n| {
            n.starts_with("ppt/slides/slide") && n.ends_with(".xml")
        })?,
    };
    Ok(truncate(text))
}

// --- outside: the sandbox ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Text(String),
    /// Not a format Steward reads, or the file could not be read.
    Failed,
    /// The extractor ran past its time and was stopped.
    TimedOut,
}

impl Outcome {
    pub fn status(&self) -> &'static str {
        match self {
            Outcome::Text(_) => "ok",
            Outcome::Failed => "failed",
            Outcome::TimedOut => "timeout",
        }
    }
}

pub struct Sandbox {
    bwrap: PathBuf,
    /// The program run inside: the indexer itself, as `… extract FILE`.
    helper: PathBuf,
    pub timeout: Duration,
    /// Tests only: files with this extension are given to this program
    /// (inside the same sandbox) instead, as `PROGRAM FILE`.
    pub test_helpers: Vec<(String, PathBuf)>,
}

impl Sandbox {
    /// None when `bwrap` is not installed: then nothing is extracted.
    pub fn new(helper: PathBuf) -> Option<Self> {
        let bwrap = glib::find_program_in_path("bwrap")?;
        Some(Self {
            bwrap,
            helper,
            timeout: Duration::from_secs(20),
            test_helpers: Vec::new(),
        })
    }

    fn test_helper(&self, path: &Path) -> Option<&Path> {
        let ext = path.extension()?.to_str()?;
        self.test_helpers
            .iter()
            .find(|(e, _)| e == ext)
            .map(|(_, p)| p.as_path())
    }

    /// Whether a file is one this sandbox would read.
    pub fn reads(&self, path: &Path) -> bool {
        format_of(path).is_some() || self.test_helper(path).is_some()
    }

    /// The bwrap command for one file (ADR 0005 profile).
    pub fn command(&self, input: &Path) -> Command {
        let ext = input
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        let inner = format!("/tmp/input{ext}");
        let mut command = Command::new(&self.bwrap);
        command.args(["--unshare-all", "--die-with-parent", "--new-session"]);
        command.args(["--ro-bind", "/usr", "/usr"]);
        command.args([
            "--symlink",
            "usr/lib",
            "/lib",
            "--symlink",
            "usr/lib64",
            "/lib64",
        ]);
        command.args([
            "--symlink",
            "usr/bin",
            "/bin",
            "--symlink",
            "usr/sbin",
            "/sbin",
        ]);
        command.args(["--ro-bind-try", "/etc/ld.so.cache", "/etc/ld.so.cache"]);
        command.args(["--ro-bind-try", "/etc/fonts", "/etc/fonts"]);
        command.args(["--dev", "/dev", "--tmpfs", "/tmp"]);
        command.arg("--ro-bind").arg(input).arg(&inner);
        command.args(["--chdir", "/tmp", "--clearenv"]);
        command.args([
            "--setenv",
            "PATH",
            "/usr/bin:/bin",
            "--setenv",
            "LANG",
            "C.UTF-8",
        ]);
        command.args(["--setenv", "HOME", "/tmp"]);
        match self.test_helper(input) {
            Some(helper) => {
                command.arg("--ro-bind").arg(helper).arg("/tmp/helper");
                command.args(["/tmp/helper", &inner]);
            }
            None => {
                command
                    .arg("--ro-bind")
                    .arg(&self.helper)
                    .arg("/tmp/steward-extract");
                command.args(["/tmp/steward-extract", "extract", &inner]);
            }
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        command
    }

    /// Extracts one file's text in the sandbox.
    pub fn extract(&self, input: &Path) -> Outcome {
        let Ok(mut child) = self.command(input).spawn() else {
            return Outcome::Failed;
        };
        let mut stdout = child.stdout.take().expect("piped");
        let reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout
                .by_ref()
                .take(TEXT_LIMIT as u64 + 1)
                .read_to_end(&mut bytes);
            bytes
        });
        let deadline = Instant::now() + self.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(15)),
                Err(_) => break None,
            }
        };
        let bytes = reader.join().unwrap_or_default();
        match status {
            None => Outcome::TimedOut,
            Some(s) if s.success() => {
                Outcome::Text(truncate(String::from_utf8_lossy(&bytes).into_owned()))
            }
            Some(_) => Outcome::Failed,
        }
    }
}

/// The line of `text` that contains `query` (case-insensitive), shortened
/// around the match, with the match's byte range in the returned line.
pub fn matching_line(text: &str, query: &str) -> Option<(String, std::ops::Range<usize>)> {
    let words: Vec<String> = words(query).map(|w| w.to_lowercase()).collect();
    let first = words.first()?;
    let line = text
        .lines()
        .find(|l| {
            let lower = l.to_lowercase();
            words.iter().all(|w| lower.contains(w.as_str()))
        })
        .or_else(|| {
            text.lines()
                .find(|l| l.to_lowercase().contains(first.as_str()))
        })?;
    let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = line.to_lowercase();
    // Lower-casing can change byte lengths; fall back to no highlight then.
    let at = lower
        .find(first.as_str())
        .filter(|_| lower.len() == line.len())?;
    let (start, end) = (at, at + first.len());
    // About 70 characters either side.
    let mut from = start.saturating_sub(70);
    while !line.is_char_boundary(from) {
        from -= 1;
    }
    let mut to = (end + 70).min(line.len());
    while !line.is_char_boundary(to) {
        to += 1;
    }
    let mut shown = String::new();
    if from > 0 {
        shown.push('…');
    }
    let offset = shown.len() as isize - from as isize;
    shown.push_str(&line[from..to]);
    if to < line.len() {
        shown.push('…');
    }
    let range = (start as isize + offset) as usize..(end as isize + offset) as usize;
    Some((shown, range))
}

/// The words of a query, as the content index splits them.
pub fn words(query: &str) -> impl Iterator<Item = &str> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_by_extension() {
        assert_eq!(format_of(Path::new("a/Report.PDF")), Some(Format::Pdf));
        assert_eq!(format_of(Path::new("notes.md")), Some(Format::Text));
        assert_eq!(format_of(Path::new("x.docx")), Some(Format::Docx));
        assert_eq!(format_of(Path::new("photo.jpg")), None);
    }

    #[test]
    fn xml_paragraphs_become_lines() {
        let xml = "<w:document><w:p><w:r><w:t>Hello &amp; welcome</w:t></w:r></w:p><w:p><w:t>Second</w:t></w:p></w:document>";
        assert_eq!(xml_text(xml), "Hello & welcome\nSecond\n");
    }

    #[test]
    fn the_matching_line_is_shortened_around_the_word() {
        let text = format!(
            "first line\n{} the Zephyr clause applies {}\nlast",
            "a".repeat(100),
            "b".repeat(100)
        );
        let (line, range) = matching_line(&text, "zephyr").unwrap();
        assert_eq!(&line[range.clone()], "Zephyr");
        assert!(line.starts_with('…') && line.ends_with('…'));
        assert!(line.chars().count() < 160);
        assert_eq!(matching_line("nothing here", "zephyr"), None);
    }
}
