//! Find anything (Ctrl+K, P3-T07 to P3-T09): the search overlay's state.
//! Queries run against the index a moment after the last keystroke; an
//! older query never overwrites a newer one. With Contents on (P7-T09),
//! files whose text has the words are found too, each with its line.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use cairn_core::Kind;
use cairn_core::index::Hit;

use super::{Controller, Event, dirty, later};
use crate::{format, paths, tint};

/// Results kept per query.
const LIMIT: usize = 120;
/// Typing pause before a query runs.
const SETTLE: Duration = Duration::from_millis(60);

#[derive(Default, Debug)]
pub struct SearchState {
    pub open: bool,
    query: String,
    /// "all" or a kind id.
    kind: String,
    hits: Vec<Hit>,
    selected: usize,
    /// Bumped per keystroke; a query that finds a newer number is dropped.
    ticket: u64,
    ran: String,
    /// Also search inside files.
    pub contents: bool,
    /// For results found by their text: the line, and where the word is.
    lines: HashMap<PathBuf, (String, std::ops::Range<usize>)>,
}

fn section_of(hit: &Hit) -> usize {
    match hit.kind.as_str() {
        _ if hit.is_dir => 0,
        "images" | "videos" => 1,
        _ => 2,
    }
}

/// A matched line for QML's styled text: escaped, the word in bold.
fn line_markup(line: &str, at: &std::ops::Range<usize>) -> String {
    let escape = |t: &str| {
        t.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    format!(
        "{}<b>{}</b>{}",
        escape(&line[..at.start]),
        escape(&line[at.clone()]),
        escape(&line[at.end..])
    )
}

/// The freedesktop thumbnail of a file, if one was made already.
fn cached_thumbnail(path: &std::path::Path) -> Option<String> {
    let uri = gio::File::for_path(path).uri();
    let md5 = glib::compute_checksum_for_string(glib::ChecksumType::Md5, uri.as_str())?;
    let cache = glib::user_cache_dir().join("thumbnails");
    ["large", "normal", "x-large"]
        .iter()
        .map(|size| cache.join(size).join(format!("{md5}.png")))
        .find(|p| p.exists())
        .map(|p| p.display().to_string())
}

impl Controller {
    pub fn open_search(&mut self) {
        self.search.open = true;
        self.search.contents = crate::settings::get().boolean("search-contents");
        if self.search.kind.is_empty() {
            self.search.kind = "all".into();
        }
        self.mark(dirty::SEARCH);
        self.emit(Event::Focus("search"));
    }

    pub fn close_search(&mut self) {
        if self.search.open {
            self.search.open = false;
            self.mark(dirty::SEARCH);
            self.emit(Event::Focus("content"));
        }
    }

    pub fn set_search_query(&mut self, text: &str) {
        if self.search.query == text {
            return;
        }
        self.search.query = text.to_string();
        self.search.ticket += 1;
        let ticket = self.search.ticket;
        glib::timeout_add_local_once(SETTLE, move || {
            later(move |c| {
                if c.search.ticket == ticket {
                    c.run_search();
                }
            })
        });
    }

    /// The index changed: run the query again so results stay true.
    pub fn search_again(&mut self) {
        if self.search.open && !self.search.query.is_empty() {
            self.run_search();
        }
    }

    pub fn search_contents(&mut self, on: bool) {
        self.search.contents = on;
        self.search_again();
        self.mark(dirty::SEARCH);
    }

    fn run_search(&mut self) {
        let query = self.search.query.clone();
        let mut lines = HashMap::new();
        let hits = if self.index.running {
            self.index
                .reader()
                .map(|r| {
                    let mut hits = r.search(&query, LIMIT);
                    if self.search.contents {
                        for found in r.search_content(&query, LIMIT) {
                            if !hits.iter().any(|h| h.path == found.hit.path) {
                                hits.push(found.hit.clone());
                            }
                            lines.insert(found.hit.path, (found.line, found.at));
                        }
                    }
                    hits
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        // The same query run again (the index changed): the chosen result
        // stays chosen if it is still there. A new query starts at the top.
        let keep = (self.search.ran == query).then(|| self.chosen()).flatten();
        self.search.lines = lines;
        self.search.hits = hits;
        self.search.ran = query;
        self.search.selected = keep
            .and_then(|path| self.shown().iter().position(|h| h.path == path))
            .unwrap_or(0);
        self.mark(dirty::SEARCH);
    }

    pub fn search_kind(&mut self, kind: &str) {
        self.search.kind = kind.to_string();
        self.search.selected = 0;
        self.mark(dirty::SEARCH);
    }

    /// The results shown, in display order (sections, then rank).
    fn shown(&self) -> Vec<&Hit> {
        let mut shown: Vec<&Hit> = self
            .search
            .hits
            .iter()
            .filter(|h| {
                self.search.kind == "all"
                    || self.search.kind.is_empty()
                    || h.kind == self.search.kind
            })
            .collect();
        shown.sort_by_key(|h| section_of(h));
        shown
    }

    pub fn search_move(&mut self, key: &str) {
        let count = self.shown().len();
        if count == 0 {
            return;
        }
        let at = self.search.selected as i64;
        let next = match key {
            "up" | "left" => at - 1,
            "down" | "right" => at + 1,
            "home" => 0,
            "end" => count as i64 - 1,
            _ => at,
        };
        self.search.selected = next.clamp(0, count as i64 - 1) as usize;
        self.mark(dirty::SEARCH);
    }

    pub fn search_select(&mut self, index: usize) {
        self.search.selected = index;
        self.mark(dirty::SEARCH);
    }

    /// The paths shown, in order (Peek moves through them).
    pub fn search_paths(&self) -> Vec<PathBuf> {
        if self.search.open {
            self.shown().iter().map(|h| h.path.clone()).collect()
        } else {
            Vec::new()
        }
    }

    fn chosen(&self) -> Option<PathBuf> {
        self.shown()
            .get(self.search.selected)
            .map(|h| h.path.clone())
    }

    /// Enter: a folder opens here; a file opens in its app.
    pub fn search_open(&mut self) {
        let Some(path) = self.chosen() else { return };
        self.close_search();
        let file = gio::File::for_path(&path);
        if path.is_dir() {
            self.navigate(&file, true);
        } else {
            let uri = file.uri().to_string();
            glib::spawn_future_local(async move {
                if let Err(error) =
                    gio::AppInfo::launch_default_for_uri_future(&uri, gio::AppLaunchContext::NONE)
                        .await
                {
                    glib::g_warning!("cairn", "could not open {uri}: {error}");
                }
            });
        }
    }

    /// Ctrl+Enter: show the item in its folder, selected.
    pub fn search_reveal(&mut self) {
        let Some(path) = self.chosen() else { return };
        let Some(parent) = path.parent() else { return };
        self.close_search();
        self.navigate(&gio::File::for_path(parent), true);
        self.select_when_shown(&gio::File::for_path(&path).uri());
    }

    /// Space: Peek at the chosen result.
    pub fn search_peek(&mut self) {
        if let Some(path) = self.chosen() {
            self.peek_path(&path);
        }
    }

    pub fn search_json(&self) -> Value {
        let shown = self.shown();
        let mut chips = vec![json!({
            "id": "all", "label": gettext("All"), "count": self.search.hits.len(),
            "active": self.search.kind == "all",
        })];
        let mut kinds: Vec<&str> = Vec::new();
        for hit in &self.search.hits {
            if !kinds.contains(&hit.kind.as_str()) {
                kinds.push(&hit.kind);
            }
        }
        kinds.sort_by_key(|k| Kind::from_id(k).unwrap_or(Kind::Other));
        for kind in kinds {
            let label = Kind::from_id(kind)
                .map(format::kind)
                .unwrap_or_else(|| kind.to_string());
            chips.push(json!({
                "id": kind, "label": label,
                "count": self.search.hits.iter().filter(|h| h.kind == kind).count(),
                "active": self.search.kind == kind,
            }));
        }
        let titles = [
            gettext("Folders"),
            gettext("Images and videos"),
            gettext("Documents and files"),
        ];
        let mut sections: Vec<Value> = Vec::new();
        for (section, title) in titles.iter().enumerate() {
            let items: Vec<Value> = shown
                .iter()
                .enumerate()
                .filter(|(_, h)| section_of(h) == section)
                .map(|(i, h)| {
                    let kind = Kind::from_id(&h.kind).unwrap_or(Kind::Other);
                    let place = h.path.parent().map(paths::display_path).unwrap_or_default();
                    let meta = if h.is_dir {
                        format!("{place} · {}", format::items(h.items as u32))
                    } else {
                        format!("{place} · {}", format::size(h.size))
                    };
                    json!({
                        "index": i, "name": h.name, "dir": h.is_dir, "kind": h.kind,
                        "path": h.path.display().to_string(),
                        "where": place, "meta": meta,
                        "date": format::date(h.mtime),
                        "badge": tint::badge(&h.name, kind),
                        "thumb": if section == 1 { cached_thumbnail(&h.path) } else { None },
                        "selected": i == self.search.selected,
                        "line": self.search.lines.get(&h.path).map(|(line, at)| line_markup(line, at)),
                    })
                })
                .collect();
            if !items.is_empty() {
                sections.push(json!({ "title": title, "items": items }));
            }
        }
        json!({
            "open": self.search.open,
            "query": self.search.query,
            "ran": self.search.ran,
            "contents": self.search.contents,
            "running": self.index.running,
            "status": self.index_status_text(),
            "chips": if self.search.hits.is_empty() { Vec::new() } else { chips },
            "sections": sections,
            "count": shown.len(),
            "selected": self.search.selected,
            "summary": if self.search.ran.is_empty() { String::new() } else {
                format!("{} {} “{}”", format::items(shown.len() as u32).replace(
                    &gettext("items"), &gettext("results")).replace(&gettext("item"), &gettext("result")),
                    gettext("for"), self.search.ran)
            },
        })
    }

    #[cfg(feature = "e2e")]
    pub fn search_test_json(&self) -> Value {
        let shown = self.shown();
        json!({
            "open": self.search.open,
            "query": self.search.query,
            "ran": self.search.ran,
            "contents": self.search.contents,
            "results": shown.iter().map(|h| {
                let line = self.search.lines.get(&h.path);
                json!({
                    "name": h.name, "path": h.path.display().to_string(), "dir": h.is_dir, "kind": h.kind,
                    "line": line.map(|(l, _)| l.clone()),
                    "match": line.map(|(l, at)| l[at.clone()].to_string()),
                })
            }).collect::<Vec<_>>(),
            "lines_markup": shown.iter().filter_map(|h| self.search.lines.get(&h.path))
                .map(|(l, at)| line_markup(l, at)).collect::<Vec<_>>(),
            "chips": self.search_json()["chips"].clone(),
            "selected": self.search.selected,
            "running": self.index.running,
        })
    }
}
