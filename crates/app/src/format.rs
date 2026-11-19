//! Human-readable sizes, dates, counts and kind names.

use gettextrs::{gettext, ngettext};

use cairn_core::Kind;

pub fn size(bytes: u64) -> String {
    glib::format_size(bytes).to_string()
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
