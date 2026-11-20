//! Smart Clean in the window (P5-T02 to P5-T07; reference: smart-clean.gif):
//! the disk overview, the cards in two groups, the review page with its
//! sticky bar, Move to Trash with Undo, and the "Needs administrator" rows.
//! Everything is worked out on background threads; nothing changes until
//! the user moves ticked items to the Trash.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use cairn_core::clean::admin::AdminRow;
use cairn_core::clean::disk::Disk;
use cairn_core::clean::{self, Card, Context, Group, Item, Reason};
use cairn_core::ops::Request;

use super::{Controller, Event, dirty, later};
use crate::{format, paths};

#[derive(Default)]
pub struct CleanState {
    /// The card under review (None: the overview).
    review: Option<String>,
    /// Cards in registry order; None while still looking.
    cards: Vec<(&'static str, Group, Option<Vec<Item>>)>,
    disk: Option<Disk>,
    admin: Option<Vec<AdminRow>>,
    /// Ticks the user changed, by path.
    overrides: HashMap<PathBuf, bool>,
    scanned: bool,
    ticket: u64,
    running: Option<Arc<AtomicBool>>,
    /// The Trash batch this page started: (summary, items, bytes). Only one
    /// batch runs at a time, so the next Trash report is this one.
    pending: Option<(String, usize, u64)>,
    /// The last command copied (tests read it when the clipboard is private).
    pub copied: Option<String>,
}

impl std::fmt::Debug for CleanState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CleanState")
            .field("review", &self.review)
            .finish()
    }
}

/// Posts `f` to the main loop from a worker thread.
fn post(f: impl FnOnce(&mut Controller) + Send + 'static) {
    glib::MainContext::default().invoke(move || later(f));
}

fn card_text(id: &str) -> (String, String, &'static str, &'static str) {
    // (title, explanation, symbol, colour)
    match id {
        "dev_caches" => (
            gettext("Developer caches"),
            gettext(
                "Only places their tools rebuild by themselves. You lose nothing but time: the next build or install is slower.",
            ),
            "code",
            "archives",
        ),
        "duplicates" => (
            gettext("Duplicates"),
            gettext(
                "The same contents in more than one place. One copy is always kept, a filed one first. Only copies that look spare are ticked: in Downloads or on the Desktop, or named like a copy.",
            ),
            "copy",
            "audio",
        ),
        "installers" => (
            gettext("Installers"),
            gettext(
                "Installer packages for apps that are already installed. Ticked only when they are in Downloads or on the Desktop, more than a week old, and not newer than what is installed.",
            ),
            "package",
            "folders",
        ),
        "stale_builds" => (
            gettext("Old build output"),
            gettext(
                "node_modules, target, .venv and build folders in projects nobody has touched for six months. The project rebuilds them when you work on it again.",
            ),
            "box",
            "code",
        ),
        "leftovers" => (
            gettext("App leftovers"),
            gettext(
                "Data that Flatpak and Snap apps left behind after they were removed, untouched for three months.",
            ),
            "trash",
            "installers",
        ),
        "big_files" => (
            gettext("Big files"),
            gettext(
                "The largest files in your home folder, disk images included. Only you know whether you still need them.",
            ),
            "drive",
            "documents",
        ),
        "screencasts" => (
            gettext("Old screen recordings"),
            gettext("Screen recordings older than 90 days."),
            "videos",
            "videos",
        ),
        _ => (
            gettext("Old downloads"),
            gettext("Downloads nobody has opened, changed or added for 90 days."),
            "downloads",
            "images",
        ),
    }
}

fn card_subtitle(id: &str, items: &[Item]) -> String {
    let n = items.iter().filter(|i| !i.keep).count() as u32;
    match id {
        "dev_caches" => gettext("npm, pip, Cargo and more"),
        "duplicates" => format!("{} {}", format::items(n), gettext("you have twice")),
        "installers" => gettext("For apps you already have"),
        "stale_builds" => gettext("In projects untouched for 6 months"),
        "leftovers" => gettext("From apps no longer installed"),
        "big_files" => format!("{} {}", format::files(n), gettext("over 256 MB")),
        "screencasts" => gettext("Older than 90 days"),
        _ => gettext("Not opened for 90 days"),
    }
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The words for an item's reason.
fn why(item: &Item) -> String {
    match &item.reason {
        Reason::Cache { cache } => gettext(cache.reason.as_str()),
        Reason::KeptCopy => gettext("Kept"),
        Reason::SameAs { kept } => format!(
            "{} {}",
            gettext("Kept: the one in"),
            kept.parent().map(paths::display_path).unwrap_or_default()
        ),
        Reason::Installed { package, version } => {
            format!("{package} {version} {}", gettext("is installed"))
        }
        Reason::StaleBuild { project, days } => format!(
            "{} {} {} {}",
            name_of(project),
            gettext("untouched for"),
            days / 30,
            gettext("months")
        ),
        Reason::Leftover { tool, .. } => match *tool {
            "snap" => gettext("The Snap is no longer installed"),
            _ => gettext("The Flatpak is no longer installed"),
        },
        Reason::Big => {
            let kind = cairn_core::plan::kind_of(&name_of(&item.path));
            gettext(kind.label())
        }
        Reason::OldRecording { days } => format!("{days} {}", gettext("days old")),
        Reason::OldDownload { days, opened } => format!(
            "{} {days} {}",
            if *opened {
                gettext("Not opened for")
            } else {
                gettext("Not changed for")
            },
            gettext("days")
        ),
    }
}

fn item_title(item: &Item) -> String {
    match &item.reason {
        Reason::Cache { cache } => gettext(cache.name.as_str()),
        Reason::StaleBuild { project, .. } => {
            format!("{} · {}", name_of(project), name_of(&item.path))
        }
        _ => name_of(&item.path),
    }
}

impl Controller {
    pub fn clean_opened(&mut self) {
        if !self.clean.scanned {
            self.clean_look_again();
        }
    }

    /// Looks again: every card back to "Looking…", then each fills in as
    /// its suggester finishes.
    pub fn clean_look_again(&mut self) {
        if let Some(running) = self.clean.running.take() {
            running.store(false, Ordering::Relaxed);
        }
        let running = Arc::new(AtomicBool::new(true));
        self.clean.running = Some(running.clone());
        self.clean.ticket += 1;
        self.clean.scanned = true;
        let ticket = self.clean.ticket;
        self.clean.cards = clean::registry()
            .iter()
            .map(|s| (s.id(), s.group(), None))
            .collect();
        self.clean.disk = None;
        self.clean.admin = None;
        self.mark(dirty::TOOLS);

        let home = glib::home_dir();
        let ctx = Arc::new(Context::for_home(
            home.clone(),
            super::downloads_folder(),
            Arc::new(cairn_core::suggest::Dpkg),
            Arc::new(cairn_core::tools::System),
        ));
        let keep = running.clone();
        clean::scan(
            ctx,
            Arc::new(move || keep.load(Ordering::Relaxed)),
            Arc::new(move |card: Card| {
                // Test hook (e2e builds): hold each card back so "Looking…"
                // can be seen (E5-01).
                #[cfg(feature = "e2e")]
                if let Some(ms) = std::env::var("CAIRN_CLEAN_DELAY_MS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                {
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                }
                post(move |c| c.clean_card_found(ticket, card));
            }),
        );
        self.clean_measure_disk();
        std::thread::spawn(move || {
            let rows = clean::admin::rows(&cairn_core::tools::System);
            post(move |c| {
                if c.clean.ticket == ticket {
                    c.clean.admin = Some(rows);
                    c.mark(dirty::TOOLS);
                }
            });
        });
    }

    fn clean_measure_disk(&mut self) {
        let ticket = self.clean.ticket;
        let home = glib::home_dir();
        let trash = cairn_core::ops::Trash::default_home().join("files");
        std::thread::spawn(move || {
            let disk = clean::disk::overview(&home, &trash, &cairn_core::tools::System);
            post(move |c| {
                if c.clean.ticket == ticket {
                    c.clean.disk = Some(disk);
                    c.mark(dirty::TOOLS | dirty::SIDEBAR);
                }
            });
        });
    }

    fn clean_card_found(&mut self, ticket: u64, card: Card) {
        if self.clean.ticket != ticket {
            return;
        }
        if let Some(slot) = self.clean.cards.iter_mut().find(|s| s.0 == card.id) {
            slot.2 = Some(card.items);
        }
        self.mark(dirty::TOOLS);
    }

    /// The cards as shown: found ones, each path in only one card.
    fn clean_cards(&self) -> Vec<(Card, bool)> {
        let mut cards: Vec<Card> = self
            .clean
            .cards
            .iter()
            .map(|(id, group, items)| Card {
                id,
                group: *group,
                items: items.clone().unwrap_or_default(),
            })
            .collect();
        clean::exclusive(&mut cards);
        cards
            .into_iter()
            .zip(self.clean.cards.iter().map(|s| s.2.is_some()))
            .collect()
    }

    fn ticked(&self, item: &Item) -> bool {
        !item.keep && *self.clean.overrides.get(&item.path).unwrap_or(&item.ticked)
    }

    pub fn clean_review(&mut self, id: &str) {
        self.clean.review = (!id.is_empty()).then(|| id.to_string());
        self.mark(dirty::TOOLS);
    }

    /// Ticks or unticks one item ("path"), or every item of a card ("card:id").
    pub fn clean_tick(&mut self, what: &str) {
        let cards = self.clean_cards();
        if let Some(id) = what.strip_prefix("card:") {
            let Some((card, _)) = cards.iter().find(|(c, _)| c.id == id) else {
                return;
            };
            let all = card
                .items
                .iter()
                .filter(|i| !i.keep)
                .all(|i| self.ticked(i));
            for item in card.items.iter().filter(|i| !i.keep) {
                self.clean.overrides.insert(item.path.clone(), !all);
            }
        } else {
            let path = PathBuf::from(what);
            let Some(item) = cards
                .iter()
                .flat_map(|(c, _)| &c.items)
                .find(|i| i.path == path)
            else {
                return;
            };
            if item.keep {
                return;
            }
            let now = self.ticked(item);
            self.clean.overrides.insert(path, !now);
        }
        self.mark(dirty::TOOLS);
    }

    /// Move to Trash: the ticked items of one card (or of every card, for
    /// ""), as one batch with Undo.
    pub fn clean_trash(&mut self, id: &str) {
        let id = if id.is_empty() {
            self.clean.review.clone().unwrap_or_default()
        } else {
            id.to_string()
        };
        let cards = self.clean_cards();
        let items: Vec<&Item> = cards
            .iter()
            .filter(|(c, _)| id == "all" || id.is_empty() || c.id == id)
            .flat_map(|(c, _)| &c.items)
            .filter(|i| self.ticked(i))
            .collect();
        if items.is_empty() {
            return;
        }
        let bytes: u64 = items.iter().map(|i| i.bytes).sum();
        let sources: Vec<PathBuf> = items.iter().map(|i| i.path.clone()).collect();
        let summary = format!(
            "{} {} {}",
            gettext("Move"),
            format::items(sources.len() as u32),
            gettext("to the Trash")
        );
        self.clean.pending = Some((summary.clone(), sources.len(), bytes));
        self.submit_request(Request::Trash { sources }, summary);
    }

    /// The toast for a batch this page started (None: not ours).
    pub fn clean_report_text(&mut self, report: &cairn_core::ops::Report) -> Option<String> {
        let (_, count, bytes) = self.clean.pending.clone()?;
        if report.kind != "trash" || report.outcome != cairn_core::ops::Outcome::Forward {
            return None;
        }
        self.clean.pending = None;
        if report.done < count {
            return None;
        }
        Some(format!(
            "{} {} ({}) {}",
            gettext("Moved"),
            format::items(count as u32),
            format::size(bytes),
            gettext("to the Trash")
        ))
    }

    /// After any file operation: drop what is gone; Undo brings items back,
    /// so look again then.
    pub fn clean_after_op(&mut self, undo: bool) {
        if !self.clean.scanned {
            return;
        }
        if undo {
            self.clean_look_again();
            return;
        }
        for slot in &mut self.clean.cards {
            if let Some(items) = &mut slot.2 {
                items.retain(|i| i.path.symlink_metadata().is_ok());
                // A duplicate set with only its kept copy left is no set.
                if slot.0 == "duplicates" {
                    let spare: Vec<PathBuf> = items
                        .iter()
                        .filter_map(|i| match &i.reason {
                            Reason::SameAs { kept } => Some(kept.clone()),
                            _ => None,
                        })
                        .collect();
                    items.retain(|i| !i.keep || spare.contains(&i.path));
                }
            }
        }
        self.clean_measure_disk();
        self.mark(dirty::TOOLS);
    }

    /// Copy on a "Needs administrator" row: the command, never run.
    pub fn clean_copy(&mut self, id: &str) {
        let Some(row) = self
            .clean
            .admin
            .as_ref()
            .and_then(|rows| rows.iter().find(|r| r.id == id))
        else {
            return;
        };
        let command = row.command.clone();
        crate::bridge::qobject::clipboard_set(
            &vec!["text/plain;charset=utf-8".into(), "text/plain".into()],
            &vec![command.clone(), command.clone()],
        );
        self.clean.copied = Some(command);
        self.emit(Event::Toast(
            gettext("Copied the command. Paste it into a terminal to run it."),
            false,
        ));
    }

    /// Show in folder, from the review page.
    pub fn clean_show(&mut self, path: &str) {
        let path = PathBuf::from(path);
        let Some(parent) = path.parent() else { return };
        self.close_tool();
        self.navigate(&gio::File::for_path(parent), true);
        self.select_when_shown(&gio::File::for_path(&path).uri());
    }

    /// Peek, from the review page: the arrows move through the card's items.
    pub fn clean_peek(&mut self, path: &str) {
        let path = PathBuf::from(path);
        let id = self.clean.review.clone().unwrap_or_default();
        let list: Vec<PathBuf> = self
            .clean_cards()
            .into_iter()
            .find(|(c, _)| c.id == id)
            .map(|(c, _)| c.items.into_iter().map(|i| i.path).collect())
            .unwrap_or_else(|| vec![path.clone()]);
        let at = list.iter().position(|p| *p == path).unwrap_or(0);
        self.peek_list(list, at);
    }

    fn disk_json(&self) -> Value {
        let Some(disk) = &self.clean.disk else {
            return json!({ "looking": true });
        };
        let used = disk.used().max(1);
        let mut parts = vec![
            ("home", gettext("Your files"), disk.home, "folders"),
            ("apps", gettext("Apps"), disk.apps.unwrap_or(0), "documents"),
            ("trash", gettext("In the Trash"), disk.trash, "pdfs"),
            (
                "other",
                gettext("System and other"),
                disk.other(),
                "installers",
            ),
        ];
        parts.retain(|p| p.0 != "apps" || disk.apps.is_some());
        // Apparent sizes of sparse files can add up to more than is used:
        // the bar shows shares of the used space.
        let sum: u64 = parts.iter().map(|p| p.2).sum::<u64>().max(used);
        let used_share = disk.used() as f64 / disk.size.max(1) as f64;
        json!({
            "looking": false,
            "label": gettext("Home disk"),
            "size": disk.size, "free": disk.free, "used": disk.used(), "trash": disk.trash,
            "freeText": format::size(disk.free),
            "sizeText": format!("{} {}", gettext("free of"), format::size(disk.size)),
            "usedText": format!("{} {}", format::size(disk.used()), gettext("used")),
            "usedShare": used_share,
            "parts": parts.iter().map(|(id, label, bytes, color)| json!({
                "id": id, "label": label, "bytes": bytes, "text": format::size(*bytes),
                "share": used_share * (*bytes as f64 / sum as f64), "color": color,
            })).collect::<Vec<_>>(),
        })
    }

    fn admin_json(&self) -> Value {
        let Some(rows) = &self.clean.admin else {
            return json!({ "looking": true, "rows": [] });
        };
        let text = |row: &AdminRow| -> (String, String) {
            match row.id {
                "apt-cache" => (
                    gettext("Package downloads (apt)"),
                    gettext("Packages apt keeps after installing them"),
                ),
                "snap-revisions" => (
                    gettext("Old Snap revisions"),
                    format!(
                        "{} {}",
                        row.count,
                        gettext("disabled revisions kept for rollback")
                    ),
                ),
                "journal" => (
                    gettext("System journal"),
                    gettext("Logs over 200 MB; the newest are kept"),
                ),
                "old-kernels" => (
                    gettext("Old kernels"),
                    format!(
                        "{} {}",
                        row.count,
                        gettext("not running and not the newest")
                    ),
                ),
                _ => (
                    gettext("Unused Docker images"),
                    gettext("Images no container uses"),
                ),
            }
        };
        json!({
            "looking": false,
            "rows": rows.iter().map(|r| {
                let (title, detail) = text(r);
                json!({
                    "id": r.id, "title": title, "detail": detail, "command": r.command,
                    "bytes": r.bytes,
                    "sizeText": r.bytes.map(format::size).unwrap_or_else(|| gettext("size unknown")),
                })
            }).collect::<Vec<_>>(),
        })
    }

    pub fn clean_json(&self) -> Value {
        let cards = self.clean_cards();
        let group_total = |group: Group| -> u64 {
            cards
                .iter()
                .filter(|(c, _)| c.group == group)
                .map(|(c, _)| c.total())
                .sum()
        };
        let ticked_bytes: u64 = cards
            .iter()
            .flat_map(|(c, _)| &c.items)
            .filter(|i| self.ticked(i))
            .map(|i| i.bytes)
            .sum();
        let ticked_count = cards
            .iter()
            .flat_map(|(c, _)| &c.items)
            .filter(|i| self.ticked(i))
            .count();
        let looking = cards.iter().any(|(_, found)| !found);
        let card_json: Vec<Value> = cards
            .iter()
            .map(|(card, found)| {
                let (title, _, symbol, color) = card_text(card.id);
                let ticked: Vec<&Item> = card.items.iter().filter(|i| self.ticked(i)).collect();
                json!({
                    "id": card.id, "group": card.group.id(), "title": title,
                    "subtitle": if *found { card_subtitle(card.id, &card.items) } else { gettext("Looking…") },
                    "symbol": symbol, "color": color,
                    "state": if *found { "done" } else { "looking" },
                    "total": card.total(), "totalText": format::size(card.total()),
                    "count": card.items.iter().filter(|i| !i.keep).count(),
                    "ticked": ticked.len(), "tickedBytes": ticked.iter().map(|i| i.bytes).sum::<u64>(),
                })
            })
            .collect();
        let review = self.clean.review.as_ref().and_then(|id| {
            let (card, found) = cards.iter().find(|(c, _)| c.id == id.as_str())?;
            let (title, explain, symbol, color) = card_text(card.id);
            let ticked: Vec<&Item> = card.items.iter().filter(|i| self.ticked(i)).collect();
            let tickable = card.items.iter().filter(|i| !i.keep).count();
            Some(json!({
                "id": card.id, "title": title, "explain": explain, "symbol": symbol, "color": color,
                "group": card.group.id(), "looking": !found,
                "items": card.items.iter().map(|i| json!({
                    "path": i.path.display().to_string(),
                    "uri": gio::File::for_path(&i.path).uri().to_string(),
                    "title": item_title(i),
                    "where": paths::display_path(&i.path),
                    "why": why(i),
                    "sizeText": format::size(i.bytes),
                    "ticked": self.ticked(i),
                    "keep": i.keep,
                })).collect::<Vec<_>>(),
                "tickedCount": ticked.len(),
                "allTicked": tickable > 0 && ticked.len() == tickable,
                "tickedText": format!("{} {} · {}", ticked.len(), gettext("ticked"),
                                      format::size(ticked.iter().map(|i| i.bytes).sum::<u64>())),
                "trashText": format!("{} {} {}", gettext("Move"), ticked.len(), gettext("to Trash")),
            }))
        });
        json!({
            "review": review,
            "looking": looking,
            "disk": self.disk_json(),
            "headline": format!("{} {}", gettext("You could free about"), format::size(ticked_bytes)),
            "tickedBytes": ticked_bytes,
            "tickedCount": ticked_count,
            "tickedText": format!("{} {} · {}", ticked_count, gettext("ticked"), format::size(ticked_bytes)),
            "groups": [
                { "id": "safe", "label": gettext("Safe to clear"), "totalText": format::size(group_total(Group::Safe)) },
                { "id": "your-call", "label": gettext("Your call"), "totalText": format::size(group_total(Group::YourCall)) },
            ],
            "cards": card_json,
            "admin": self.admin_json(),
            "trashText": self.clean.disk.as_ref().map(|d| format::size(d.trash)).unwrap_or_default(),
        })
    }

    #[cfg(feature = "e2e")]
    pub fn clean_test_json(&self) -> Value {
        let cards = self.clean_cards();
        json!({
            "review": self.clean.review,
            "looking": cards.iter().filter(|(_, f)| !f).map(|(c, _)| c.id).collect::<Vec<_>>(),
            "cards": cards.iter().map(|(c, found)| (c.id.to_string(), json!({
                "group": c.group.id(),
                "state": if *found { "done" } else { "looking" },
                "total": c.total(),
                "items": c.items.iter().map(|i| json!({
                    "path": i.path.display().to_string(), "bytes": i.bytes,
                    "ticked": self.ticked(i), "keep": i.keep, "why": why(i),
                })).collect::<Vec<_>>(),
            }))).collect::<serde_json::Map<_, _>>(),
            "ticked_bytes": cards.iter().flat_map(|(c, _)| &c.items).filter(|i| self.ticked(i)).map(|i| i.bytes).sum::<u64>(),
            "disk": self.clean.disk.as_ref().map(|d| json!({
                "size": d.size, "free": d.free, "home": d.home, "apps": d.apps, "trash": d.trash,
            })),
            "admin": self.clean.admin.as_ref().map(|rows| rows.iter().map(|r| json!({
                "id": r.id, "bytes": r.bytes, "command": r.command,
            })).collect::<Vec<_>>()),
            "copied": self.clean.copied,
        })
    }
}
