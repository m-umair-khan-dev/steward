//! The window's side of the index (Phase 3): starting `cairn-indexer`,
//! reading `index.db`, folder sizes and counts, the preview outline, and
//! telling the indexer about the app's own changes.

use std::path::{Path, PathBuf};

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use cairn_core::index::Index;

use super::{Controller, dirty, later};
use crate::format;

const BUS_NAME: &str = "dev.cairn.Indexer";
const OBJECT_PATH: &str = "/dev/cairn/Indexer";
const INTERFACE: &str = "dev.cairn.Indexer1";

#[derive(Default)]
pub struct IndexState {
    reader: Option<Index>,
    /// The indexer owns its bus name.
    pub running: bool,
    /// Folders whose card came on screen (URIs): their numbers are shown.
    seen: std::collections::HashSet<String>,
    /// Bumped on every index change, so views can refresh.
    pub generation: u64,
    _watch: Option<gio::WatcherId>,
}

impl std::fmt::Debug for IndexState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("IndexState")
            .field("running", &self.running)
            .finish()
    }
}

fn session() -> Option<gio::DBusConnection> {
    gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()
}

/// Where the indexer binary is: next to ours (development, tests), unless
/// CAIRN_INDEXER_BIN says otherwise.
fn indexer_binary() -> Option<PathBuf> {
    if let Some(bin) = std::env::var_os("CAIRN_INDEXER_BIN") {
        return Some(PathBuf::from(bin));
    }
    let sibling = std::env::current_exe()
        .ok()?
        .parent()?
        .join("cairn-indexer");
    sibling.exists().then_some(sibling)
}

/// Starts the indexer unless it already runs: D-Bus activation first (an
/// installed build has a service file), else our own copy as a child that
/// ends with the app.
pub fn ensure_running() {
    let Some(bus) = session() else { return };
    let owned = bus
        .call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameHasOwner",
            Some(&(BUS_NAME,).to_variant()),
            Some(glib::VariantTy::new("(b)").expect("type")),
            gio::DBusCallFlags::NONE,
            2000,
            gio::Cancellable::NONE,
        )
        .ok()
        .and_then(|v| v.child_value(0).get::<bool>())
        .unwrap_or(false);
    if owned {
        return;
    }
    let activated = bus
        .call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "StartServiceByName",
            Some(&(BUS_NAME, 0u32).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            5000,
            gio::Cancellable::NONE,
        )
        .is_ok();
    if activated {
        return;
    }
    let Some(binary) = indexer_binary() else {
        glib::g_warning!(
            "cairn",
            "cairn-indexer not found; search and folder sizes are off"
        );
        return;
    };
    let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
    launcher.setenv("CAIRN_INDEXER_CHILD", "1", true);
    if let Err(error) = launcher.spawn(&[binary.as_os_str()]) {
        glib::g_warning!("cairn", "could not start {}: {error}", binary.display());
    }
}

/// Tells the indexer about paths the app just changed (it would see them
/// anyway; this makes it immediate).
pub fn notify(paths: &[PathBuf]) {
    let Some(bus) = session() else { return };
    let paths: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if paths.is_empty() {
        return;
    }
    bus.call(
        Some(BUS_NAME),
        OBJECT_PATH,
        INTERFACE,
        "Notify",
        Some(&(paths,).to_variant()),
        None,
        gio::DBusCallFlags::NO_AUTO_START,
        2000,
        gio::Cancellable::NONE,
        |_| {},
    );
}

fn call(method: &str) {
    if let Some(bus) = session() {
        bus.call(
            Some(BUS_NAME),
            OBJECT_PATH,
            INTERFACE,
            method,
            None,
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            5000,
            gio::Cancellable::NONE,
            |_| {},
        );
    }
}

/// Called once from `controller::init`.
pub fn start(c: &mut Controller) {
    ensure_running();
    let watch = gio::bus_watch_name(
        gio::BusType::Session,
        BUS_NAME,
        gio::BusNameWatcherFlags::NONE,
        |_, _, _| later(|c| c.index_up(true)),
        |_, _| later(|c| c.index_up(false)),
    );
    c.index.watch_id(watch);
    if let Some(bus) = session() {
        // The newer subscribe_to_signal needs GLib 2.82+; 24.04 has 2.80 (ADR 0001).
        #[allow(deprecated)]
        bus.signal_subscribe(
            Some(BUS_NAME),
            Some(INTERFACE),
            Some("Changed"),
            Some(OBJECT_PATH),
            None,
            gio::DBusSignalFlags::NONE,
            |_, _, _, _, _, params| {
                let folders: Vec<String> = params.child_get(0);
                later(move |c| c.index_changed(&folders));
            },
        );
    }
}

impl IndexState {
    fn watch_id(&mut self, id: gio::WatcherId) {
        self._watch = Some(id);
    }

    /// The index, opened read-only on first use.
    pub fn reader(&mut self) -> Option<&Index> {
        if self.reader.is_none() {
            let path = Index::default_path();
            if path.exists() {
                self.reader = Index::open_read_only(&path).ok();
            }
        }
        self.reader.as_ref()
    }

    pub fn reader_ref(&self) -> Option<&Index> {
        self.reader.as_ref()
    }
}

impl Controller {
    fn index_up(&mut self, running: bool) {
        self.index.running = running;
        self.mark(dirty::COUNTS | dirty::PREVIEW | dirty::SEARCH);
    }

    fn index_changed(&mut self, folders: &[String]) {
        // After a full crawl the file may be new (rebuilt): open it again.
        if folders.iter().any(|f| f == "*") {
            self.index.reader = None;
        }
        let _ = self.index.reader();
        self.index.generation += 1;
        self.search_again();
        self.mark(dirty::COUNTS | dirty::PREVIEW);
    }

    pub fn start_indexer(&mut self) {
        ensure_running();
    }

    /// Pause or resume: the setting, and the indexer told directly.
    pub fn set_index_paused(&mut self, paused: bool) {
        let _ = crate::settings::get().set_boolean("index-paused", paused);
        call(if paused { "Pause" } else { "Resume" });
        self.mark(dirty::PREFS);
    }

    pub fn rebuild_index(&mut self) {
        call("Rescan");
    }

    /// Adds or removes a folder the index leaves out ("+path" or "-path").
    /// The indexer restarts with the new rules.
    pub fn edit_index_exclude(&mut self, change: &str) {
        let s = crate::settings::get();
        let mut list: Vec<String> = s
            .strv("index-exclude")
            .iter()
            .map(|x| x.to_string())
            .collect();
        if let Some(path) = change.strip_prefix('+') {
            let path = crate::paths::expand(path.trim(), None)
                .display()
                .to_string();
            if !path.is_empty() && !list.contains(&path) {
                list.push(path);
            }
        } else if let Some(path) = change.strip_prefix('-') {
            list.retain(|x| x != path);
        }
        let refs: Vec<&str> = list.iter().map(String::as_str).collect();
        let _ = s.set_strv("index-exclude", refs.as_slice());
        self.mark(dirty::PREFS);
        // It exits to pick up the new rules; start it again shortly.
        glib::timeout_add_local_once(std::time::Duration::from_millis(500), ensure_running);
    }

    /// A folder card came on screen: show its numbers.
    pub fn folder_seen(&mut self, uri: &str) {
        if self.index.seen.insert(uri.to_string()) {
            let _ = self.index.reader();
            self.mark(dirty::COUNTS);
        }
    }

    /// (items, bytes) of a folder from the index, if it knows it.
    pub fn folder_numbers(&self, path: &Path) -> Option<(u64, u64)> {
        if !self.index.running {
            return None;
        }
        self.index
            .reader_ref()?
            .folder_stats(path)
            .map(|s| (s.items, s.bytes))
    }

    /// Per folder card: "N items" and the size (a dash when the index does
    /// not know it, E3-12).
    pub fn counts_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        for uri in &self.index.seen {
            let Some(path) = gio::File::for_uri(uri).path() else {
                continue;
            };
            let entry = match self.folder_numbers(&path) {
                Some((items, bytes)) => json!({
                    "count": format::items(items as u32), "size": format::size(bytes),
                    "items": items, "bytes": bytes,
                }),
                None => match self.counts.get(uri) {
                    Some(n) => json!({ "count": format::items(*n), "size": "—", "items": n }),
                    None => continue,
                },
            };
            map.insert(uri.clone(), entry);
        }
        Value::Object(map)
    }

    /// The preview pane's outline of a folder, two levels deep.
    pub fn outline_json(&self, path: &Path) -> Option<Value> {
        if !self.index.running {
            return None;
        }
        let lines = self.index.reader_ref()?.outline(path, 14);
        if lines.is_empty() {
            return None;
        }
        Some(Value::Array(
            lines
                .iter()
                .map(|l| json!({
                    "name": l.name, "depth": l.depth, "dir": l.is_dir,
                    "items": if l.is_dir { format::items(l.items as u32) } else { String::new() },
                }))
                .collect(),
        ))
    }

    pub fn index_status_text(&self) -> String {
        if self.index.running {
            String::new()
        } else {
            gettext("The search index is not running.")
        }
    }

    #[cfg(feature = "e2e")]
    pub fn index_test_json(&self) -> Value {
        json!({
            "running": self.index.running,
            "generation": self.index.generation,
            "open": self.index.reader_ref().is_some(),
        })
    }
}
