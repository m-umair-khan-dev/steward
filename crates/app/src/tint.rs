//! Type badges per kind (ADR 0008). Colours live in qml/Theme.qml.

use cairn_core::Kind;

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
