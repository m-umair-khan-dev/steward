//! What kind of thing a file is, from its content type (DESIGN.md P1-T02).
//!
//! One table, used everywhere: grouping, filter chips, Tidy (Phase 4) and
//! Smart Clean (Phase 5). The fixture generator (`docker/scripts/make-fixture-home`)
//! has a matching extension table; `tests/fixture_agreement.rs` checks that the
//! two agree for every planted file.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Folder,
    Images,
    Videos,
    Audio,
    Pdfs,
    Documents,
    Archives,
    Installers,
    Code,
    Other,
}

impl Kind {
    /// Every kind, in the order groups and chips are shown.
    pub const ALL: [Kind; 10] = [
        Kind::Folder,
        Kind::Images,
        Kind::Videos,
        Kind::Audio,
        Kind::Pdfs,
        Kind::Documents,
        Kind::Archives,
        Kind::Installers,
        Kind::Code,
        Kind::Other,
    ];

    /// Stable identifier, used in settings, actions and test state.
    pub fn id(self) -> &'static str {
        match self {
            Kind::Folder => "folders",
            Kind::Images => "images",
            Kind::Videos => "videos",
            Kind::Audio => "audio",
            Kind::Pdfs => "pdfs",
            Kind::Documents => "documents",
            Kind::Archives => "archives",
            Kind::Installers => "installers",
            Kind::Code => "code",
            Kind::Other => "other",
        }
    }

    pub fn from_id(id: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.id() == id)
    }

    /// English label. The UI passes it through gettext.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Folder => "Folders",
            Kind::Images => "Images",
            Kind::Videos => "Videos",
            Kind::Audio => "Audio",
            Kind::Pdfs => "PDFs",
            Kind::Documents => "Documents",
            Kind::Archives => "Archives",
            Kind::Installers => "Installers",
            Kind::Code => "Code",
            Kind::Other => "Other",
        }
    }

    /// The kind of a content type as GIO reports it (a MIME type on Linux).
    pub fn from_content_type(content_type: &str) -> Kind {
        let ct = content_type.trim().to_ascii_lowercase();
        if ct == "inode/directory" {
            return Kind::Folder;
        }
        if ct.starts_with("image/") {
            return Kind::Images;
        }
        if ct.starts_with("video/") {
            return Kind::Videos;
        }
        if ct.starts_with("audio/") {
            return Kind::Audio;
        }
        if ct == "application/pdf" {
            return Kind::Pdfs;
        }
        if INSTALLERS.contains(&ct.as_str()) {
            return Kind::Installers;
        }
        if ARCHIVES.contains(&ct.as_str()) {
            return Kind::Archives;
        }
        if DOCUMENTS.contains(&ct.as_str())
            || ct.starts_with("application/vnd.oasis.opendocument.")
            || ct.starts_with("application/vnd.openxmlformats-officedocument.")
        {
            return Kind::Documents;
        }
        if CODE.contains(&ct.as_str())
            || (ct.starts_with("text/x-") && (ct.ends_with("src") || ct.ends_with("hdr")))
        {
            return Kind::Code;
        }
        Kind::Other
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

const INSTALLERS: &[&str] = &[
    "application/vnd.debian.binary-package",
    "application/x-deb",
    "application/x-rpm",
    "application/x-redhat-package-manager",
    "application/vnd.flatpak.ref",
    "application/vnd.flatpak",
];

const ARCHIVES: &[&str] = &[
    "application/zip",
    "application/x-zip-compressed",
    "application/x-tar",
    "application/gzip",
    "application/x-gzip",
    "application/x-compressed-tar",
    "application/x-xz",
    "application/x-xz-compressed-tar",
    "application/x-bzip",
    "application/x-bzip2",
    "application/x-bzip2-compressed-tar",
    "application/x-7z-compressed",
    "application/zstd",
    "application/x-zstd-compressed-tar",
    "application/vnd.rar",
    "application/x-rar",
    "application/x-rar-compressed",
];

const DOCUMENTS: &[&str] = &[
    "text/plain",
    "text/markdown",
    "text/x-markdown",
    "text/csv",
    "text/rtf",
    "application/rtf",
    "application/msword",
    "application/vnd.ms-excel",
    "application/vnd.ms-powerpoint",
    "application/epub+zip",
];

const CODE: &[&str] = &[
    "text/rust",
    "text/x-rust",
    "text/x-python",
    "text/x-python3",
    "text/javascript",
    "application/javascript",
    "application/x-javascript",
    "text/typescript",
    "application/json",
    "application/toml",
    "text/x-toml",
    "application/x-shellscript",
    "text/x-go",
    "text/html",
    "text/css",
    "application/xml",
    "text/xml",
    "application/x-yaml",
    "application/yaml",
    "text/x-java",
    "text/x-sql",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_types_in_the_fixture_map_to_the_fixture_kinds() {
        let cases = [
            ("inode/directory", Kind::Folder),
            ("image/png", Kind::Images),
            ("image/jpeg", Kind::Images),
            ("video/mp4", Kind::Videos),
            ("video/webm", Kind::Videos),
            ("application/pdf", Kind::Pdfs),
            ("application/vnd.oasis.opendocument.text", Kind::Documents),
            ("application/vnd.oasis.opendocument.spreadsheet", Kind::Documents),
            (
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                Kind::Documents,
            ),
            ("text/plain", Kind::Documents),
            ("text/markdown", Kind::Documents),
            ("text/csv", Kind::Documents),
            ("application/zip", Kind::Archives),
            ("application/vnd.debian.binary-package", Kind::Installers),
            ("application/json", Kind::Code),
            ("text/javascript", Kind::Code),
            ("text/rust", Kind::Code),
            ("application/toml", Kind::Code),
            ("application/vnd.appimage", Kind::Other),
            ("application/x-cd-image", Kind::Other),
            ("application/x-partial-download", Kind::Other),
            ("application/octet-stream", Kind::Other),
        ];
        for (content_type, expected) in cases {
            assert_eq!(Kind::from_content_type(content_type), expected, "{content_type}");
        }
    }

    #[test]
    fn ids_round_trip() {
        for kind in Kind::ALL {
            assert_eq!(Kind::from_id(kind.id()), Some(kind));
        }
        assert_eq!(Kind::from_id("nonsense"), None);
    }

    #[test]
    fn matching_ignores_case_and_spaces() {
        assert_eq!(Kind::from_content_type(" Image/PNG "), Kind::Images);
    }
}
