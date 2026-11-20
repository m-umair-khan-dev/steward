//! First run and updates (P6-T01, P6-T02): the welcome (a demo of a messy
//! folder being sorted, a short tour, "make it yours") shows once; after an
//! update, "What's New" shows once, from the release-notes file that also
//! feeds the website's changelog and the Debian changelog.

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use super::{Controller, dirty};
use crate::settings;

/// This version's release notes (site/src/content/changelog/<version>.md).
/// A release without notes does not build.
const NOTES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../site/src/content/changelog/",
    env!("CARGO_PKG_VERSION"),
    ".md"
));

pub const STEPS: [&str; 3] = ["demo", "tour", "yours"];

#[derive(Default, Debug)]
pub struct WelcomeState {
    step: usize,
    /// The demo folder has been sorted.
    sorted: bool,
}

/// (title, items) from a release-notes file: the front matter's title, and
/// each "- " line with **bold** kept as <b>bold</b>.
pub fn parse_notes(text: &str) -> (String, Vec<String>) {
    let mut title = String::new();
    let mut items = Vec::new();
    let mut lines = text.lines();
    if text.starts_with("---") {
        lines.next();
        for line in lines.by_ref() {
            if line.trim() == "---" {
                break;
            }
            if let Some(t) = line.strip_prefix("title:") {
                title = t.trim().to_string();
            }
        }
    }
    for line in lines {
        if let Some(item) = line.strip_prefix("- ") {
            let mut html = String::new();
            for (i, part) in item.split("**").enumerate() {
                let part = part.replace('&', "&amp;").replace('<', "&lt;");
                if i % 2 == 1 {
                    html.push_str(&format!("<b>{part}</b>"));
                } else {
                    html.push_str(&part);
                }
            }
            items.push(html);
        }
    }
    (title, items)
}

/// Dotted versions compared as numbers ("1.10.0" > "1.9.2").
pub fn newer(current: &str, stored: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap_or(0))
            .collect()
    };
    parts(current) > parts(stored)
}

/// Test builds start without the welcome unless a test asks for it
/// (`CAIRN_NO_WELCOME` empty), so other scenarios see the window at once.
fn suppressed() -> bool {
    cfg!(feature = "e2e") && std::env::var("CAIRN_NO_WELCOME").is_ok_and(|v| !v.is_empty())
}

impl Controller {
    /// At start-up: the welcome on the first run, What's New after an update.
    pub fn first_run_check(&mut self) {
        if suppressed() {
            return;
        }
        let s = settings::get();
        let current = env!("CARGO_PKG_VERSION");
        if !s.boolean("welcome-shown") {
            self.welcome = WelcomeState::default();
            self.welcome_dialog();
        } else if newer(current, &s.string("last-version")) {
            let (title, items) = parse_notes(NOTES);
            self.dialog_data = json!({ "version": current, "title": title, "items": items });
            self.show_dialog("whats-new");
        }
    }

    fn welcome_dialog(&mut self) {
        self.dialog_data = self.welcome_json();
        self.show_dialog("welcome");
        self.mark(dirty::PREFS);
    }

    pub fn welcome_step(&mut self, change: &str) {
        match change {
            "next" => self.welcome.step = (self.welcome.step + 1).min(STEPS.len() - 1),
            "back" => self.welcome.step = self.welcome.step.saturating_sub(1),
            "sort" => self.welcome.sorted = true,
            "unsort" => self.welcome.sorted = false,
            step => {
                if let Some(i) = STEPS.iter().position(|s| *s == step) {
                    self.welcome.step = i;
                }
            }
        }
        self.welcome_dialog();
    }

    /// Finishing or skipping the welcome, or closing What's New: closing
    /// records that it was seen (see `close_dialog`).
    pub fn first_run_done(&mut self) {
        if self.dialog == "welcome" || self.dialog == "whats-new" {
            self.close_dialog();
        }
    }

    pub fn welcome_json(&self) -> Value {
        json!({
            "step": STEPS[self.welcome.step],
            "index": self.welcome.step,
            "count": STEPS.len(),
            "sorted": self.welcome.sorted,
            "version": env!("CARGO_PKG_VERSION"),
            "tour": [
                { "keys": "Ctrl+K", "title": gettext("Find anything"), "text": gettext("Folders and files by name, as you type.") },
                { "keys": "Ctrl+Shift+O", "title": gettext("Tidy a folder"), "text": gettext("See the plan, then sort. One Undo puts it back.") },
                { "keys": "F3", "title": gettext("Two folders side by side"), "text": gettext("Copy or move across with F5 and F6.") },
                { "keys": "Ctrl+Shift+K", "title": gettext("Free up space"), "text": gettext("What is safe to clear, and why.") },
            ],
        })
    }

    #[cfg(feature = "e2e")]
    pub fn welcome_test_json(&self) -> Value {
        let s = settings::get();
        json!({
            "step": STEPS[self.welcome.step], "sorted": self.welcome.sorted,
            "shown": s.boolean("welcome-shown"), "last_version": s.string("last-version").as_str(),
            "notes": parse_notes(NOTES).1.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_numbers() {
        assert!(newer("1.10.0", "1.9.2"));
        assert!(newer("1.0.0", ""));
        assert!(!newer("1.0.0", "1.0.0"));
        assert!(!newer("1.0.0", "1.0.1"));
    }

    #[test]
    fn the_notes_have_a_title_and_items() {
        let (title, items) = parse_notes(NOTES);
        assert!(!title.is_empty());
        assert!(!items.is_empty());
        assert!(items[0].contains("<b>"));
    }
}
