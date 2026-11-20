//! Human-readable sizes, dates, counts and kind names.

use gettextrs::{gettext, ngettext};

use cairn_core::Kind;

pub fn size(bytes: u64) -> String {
    glib::format_size(bytes).to_string()
}

thread_local! {
    static TYPE_NAMES: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// "PDF document" for a content type, looked up once per type (the MIME
/// database lookup is slow enough to matter for 10,000 cards, P6-T14).
pub fn type_description(content_type: &str) -> String {
    TYPE_NAMES.with(|names| {
        names
            .borrow_mut()
            .entry(content_type.to_string())
            .or_insert_with(|| gio::content_type_get_description(content_type).to_string())
            .clone()
    })
}

pub fn kind(kind: Kind) -> String {
    gettext(kind.label())
}

/// A date relative to today: "Today, 09:41", "Yesterday, 18:20", "3 Oct",
/// "14 Nov 2024". Unknown dates (0) show as a dash.
pub fn date(secs: i64) -> String {
    if secs <= 0 {
        return "—".into();
    }
    let (Ok(when), Ok(now)) = (
        glib::DateTime::from_unix_local(secs),
        glib::DateTime::now_local(),
    ) else {
        return "—".into();
    };
    let same_year = when.year() == now.year();
    let days_ago = if same_year {
        now.day_of_year() - when.day_of_year()
    } else {
        i32::MAX
    };
    let pattern = match days_ago {
        0 => gettext("Today, %H:%M"),
        1 => gettext("Yesterday, %H:%M"),
        _ if same_year => gettext("%-e %b"),
        _ => gettext("%-e %b %Y"),
    };
    when.format(&pattern)
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "—".into())
}

pub fn items(n: u32) -> String {
    ngettext("{} item", "{} items", n).replace("{}", &n.to_string())
}

pub fn folders(n: u32) -> String {
    ngettext("{} folder", "{} folders", n).replace("{}", &n.to_string())
}

pub fn files(n: u32) -> String {
    ngettext("{} file", "{} files", n).replace("{}", &n.to_string())
}
