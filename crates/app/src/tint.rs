//! Colours and symbols per kind (ADR 0008). The colours themselves live in
//! `data/style.css` as `.tint-<name>` rules.

use cairn_core::Kind;

/// The CSS tint class for a kind.
pub fn class(kind: Kind) -> &'static str {
    match kind {
        Kind::Folder => "tint-purple",
        Kind::Images => "tint-orange",
        Kind::Videos => "tint-teal",
        Kind::Audio => "tint-pink",
        Kind::Pdfs => "tint-red",
        Kind::Documents => "tint-blue",
        Kind::Archives => "tint-green",
        Kind::Installers => "tint-slate",
        Kind::Code => "tint-indigo",
        Kind::Other => "tint-gray",
    }
}

pub const ALL_CLASSES: [&str; 10] = [
    "tint-purple",
    "tint-orange",
    "tint-teal",
    "tint-pink",
    "tint-red",
    "tint-blue",
    "tint-green",
    "tint-slate",
    "tint-indigo",
    "tint-gray",
];

/// A symbolic icon from the icon theme for a kind folder's glass front.
pub fn symbol(kind: Kind) -> &'static str {
    match kind {
        Kind::Folder => "folder-symbolic",
        Kind::Images => "image-x-generic-symbolic",
        Kind::Videos => "video-x-generic-symbolic",
        Kind::Audio => "audio-x-generic-symbolic",
        Kind::Pdfs => "x-office-document-symbolic",
        Kind::Documents => "x-office-document-symbolic",
        Kind::Archives => "package-x-generic-symbolic",
        Kind::Installers => "system-software-install-symbolic",
        Kind::Code => "text-x-script-symbolic",
        Kind::Other => "view-more-horizontal-symbolic",
    }
}

/// The short label on a file's type badge: its extension, upper-case.
pub fn badge(name: &str, kind: Kind) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && ext.len() <= 5 => {
            ext.to_uppercase()
        }
        _ => match kind {
            Kind::Images => "IMG".into(),
            Kind::Videos => "VID".into(),
            Kind::Pdfs => "PDF".into(),
            _ => "FILE".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badges_use_the_extension() {
        assert_eq!(badge("Invoice-0923.pdf", Kind::Pdfs), "PDF");
        assert_eq!(badge("notes.md", Kind::Documents), "MD");
        assert_eq!(badge("Makefile", Kind::Other), "FILE");
        assert_eq!(badge(".bashrc", Kind::Other), "FILE");
        assert_eq!(badge("x.verylongext", Kind::Other), "FILE");
    }
}
