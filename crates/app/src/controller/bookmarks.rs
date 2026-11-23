//! Bookmarks (P7-T08; reference: bookmarks.gif): links with pictures, in
//! collections, with tags and favourites, shown four ways (Grid, List,
//! Headlines, Masonry). Ways in: paste a link on the page, drop one onto
//! it, Add Link (Ctrl+Shift+L), and `steward add-link` (P7-T10).
//!
//! A new link's title and picture are fetched from its site, unless
//! Preferences turn that off; then nothing is requested and the card shows
//! the address.

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use steward_core::bookmarks::{self, Bookmark, Collection};

use super::{Controller, Event, dirty, later};
use crate::{app_data, format, settings};

#[derive(Default, Debug)]
pub struct BookmarksState {
    all: Vec<Bookmark>,
    collections: Vec<Collection>,
    loaded: bool,
    /// "all", "favourites" or "c<id>".
    filter: String,
    tag: Option<String>,
    /// Ids whose page is being fetched.
    fetching: Vec<i64>,
}

const VIEWS: [&str; 4] = ["grid", "list", "headlines", "masonry"];

impl Controller {
    pub fn bookmarks_load(&mut self) {
        let store = app_data::store();
        let store = store.borrow();
        self.bookmarks.all = store.bookmarks().unwrap_or_default();
        self.bookmarks.collections = store.collections().unwrap_or_default();
        self.bookmarks.loaded = true;
        self.mark(dirty::TOOLS | dirty::SIDEBAR);
    }

    fn bookmarks_ensure(&mut self) {
        if !self.bookmarks.loaded {
            self.bookmarks_load();
        }
    }

    /// Opens the page on a filter: "all", "favourites" or "c<id>".
    pub fn bookmarks_show(&mut self, filter: &str) {
        self.bookmarks_ensure();
        self.bookmarks.filter = filter.to_string();
        self.bookmarks.tag = None;
        if self.tools.page == "bookmarks" {
            self.mark(dirty::TOOLS | dirty::SIDEBAR);
        } else {
            self.open_tool("bookmarks");
        }
    }

    pub fn bookmarks_opened(&mut self) {
        self.bookmarks_ensure();
        if self.bookmarks.filter.is_empty() {
            self.bookmarks.filter = "all".into();
        }
    }

    pub fn bookmarks_tag(&mut self, tag: &str) {
        self.bookmarks.tag = (!tag.is_empty() && self.bookmarks.tag.as_deref() != Some(tag))
            .then(|| tag.to_string());
        self.mark(dirty::TOOLS);
    }

    pub fn bookmarks_view(&mut self, view: &str) {
        if VIEWS.contains(&view) {
            let _ = settings::get().set_string("bookmark-view", view);
            self.mark(dirty::TOOLS);
        }
    }

    /// Adds a link (typed, pasted, dropped or from the command line).
    pub fn bookmark_add(&mut self, text: &str) {
        let Some(url) = bookmarks::normalise(text) else {
            self.emit(Event::Toast(gettext("That is not a web link"), false));
            return;
        };
        if self.dialog == "bookmark-add" {
            self.close_dialog();
        }
        let added = app_data::store()
            .borrow()
            .bookmark_add(&url, app_data::now());
        let (id, new) = match added {
            Ok(added) => added,
            Err(error) => {
                self.emit(Event::Toast(
                    format!("{}: {error}", gettext("Could not save the link")),
                    false,
                ));
                return;
            }
        };
        // A link added inside a collection goes into it.
        if new {
            if let Some(collection) = self
                .bookmarks
                .filter
                .strip_prefix('c')
                .and_then(|c| c.parse().ok())
            {
                let _ = app_data::store()
                    .borrow()
                    .bookmark_set_collection(id, Some(collection));
            }
            if self.bookmarks.filter == "favourites" {
                let _ = app_data::store().borrow().bookmark_set_favourite(id, true);
            }
        }
        self.bookmarks_load();
        self.emit(Event::Toast(
            if new {
                gettext("Link saved")
            } else {
                gettext("Already in Bookmarks")
            },
            false,
        ));
        if new && settings::get().boolean("bookmark-fetch") {
            self.bookmark_fetch(id, url);
        }
    }

    fn bookmark_fetch(&mut self, id: i64, url: String) {
        self.bookmarks.fetching.push(id);
        self.mark(dirty::TOOLS);
        std::thread::spawn(move || {
            let meta = bookmarks::fetch(&url, &bookmarks::pictures_dir());
            glib::MainContext::default().invoke(move || {
                later(move |c| {
                    c.bookmarks.fetching.retain(|f| *f != id);
                    let _ = app_data::store().borrow().bookmark_set_meta(id, &meta);
                    c.bookmarks_load();
                })
            });
        });
    }

    /// Ctrl+V on the page: the clipboard's text, if it is a link.
    pub fn bookmarks_paste(&mut self) {
        let text = crate::bridge::qobject::clipboard_get("text/plain");
        let first = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("");
        if first.is_empty() {
            self.emit(Event::Toast(gettext("Nothing to paste"), false));
        } else {
            self.bookmark_add(first);
        }
    }

    pub fn bookmark_add_ask(&mut self) {
        self.dialog_data = json!({});
        self.show_dialog("bookmark-add");
    }

    fn bookmark_by(&self, id: &str) -> Option<Bookmark> {
        let id: i64 = id.parse().ok()?;
        self.bookmarks.all.iter().find(|b| b.id == id).cloned()
    }

    pub fn bookmark_open(&mut self, id: &str) {
        let Some(b) = self.bookmark_by(id) else {
            return;
        };
        if let Err(error) =
            gio::AppInfo::launch_default_for_uri(&b.url, None::<&gio::AppLaunchContext>)
        {
            self.emit(Event::Toast(
                format!("{}: {error}", gettext("Could not open it")),
                false,
            ));
        }
    }

    pub fn bookmark_copy(&mut self, id: &str) {
        let Some(b) = self.bookmark_by(id) else {
            return;
        };
        crate::bridge::qobject::clipboard_set(&vec!["text/plain".into()], &vec![b.url.clone()]);
        self.emit(Event::Toast(gettext("Link copied"), false));
    }

    pub fn bookmark_favourite(&mut self, id: &str) {
        let Some(b) = self.bookmark_by(id) else {
            return;
        };
        let _ = app_data::store()
            .borrow()
            .bookmark_set_favourite(b.id, !b.favourite);
        self.bookmarks_load();
    }

    pub fn bookmark_delete(&mut self, id: &str) {
        let Some(b) = self.bookmark_by(id) else {
            return;
        };
        if self.dialog == "bookmark-edit" {
            self.close_dialog();
        }
        if let Ok(Some(picture)) = app_data::store().borrow().bookmark_delete(b.id) {
            // Only pictures the app saved itself.
            if picture.starts_with(bookmarks::pictures_dir()) {
                let _ = std::fs::remove_file(picture); // E2-13 allow: the app's own picture cache
            }
        }
        self.bookmarks_load();
        self.emit(Event::Toast(gettext("Bookmark removed"), false));
    }

    pub fn bookmark_edit_ask(&mut self, id: &str) {
        let Some(b) = self.bookmark_by(id) else {
            return;
        };
        let collection = b
            .collection
            .and_then(|c| self.bookmarks.collections.iter().find(|x| x.id == c))
            .map(|c| c.name.clone())
            .unwrap_or_default();
        self.dialog_data = json!({
            "id": b.id, "url": b.url, "title": b.title.clone().unwrap_or_default(),
            "tags": b.tags.join(", "), "collection": collection,
            "collections": self.bookmarks.collections.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
        });
        self.show_dialog("bookmark-edit");
    }

    /// "id|title|tags|collection" from the Edit dialog. A collection name
    /// that does not exist yet is created.
    pub fn bookmark_edit(&mut self, value: &str) {
        let mut parts = value.splitn(4, '|');
        let (Some(id), Some(title), Some(tags), Some(collection)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return;
        };
        let Some(b) = self.bookmark_by(id) else {
            return;
        };
        {
            let store = app_data::store();
            let mut store = store.borrow_mut();
            let _ = store.bookmark_set_title(b.id, title);
            let tags: Vec<String> = tags.split(',').map(str::to_string).collect();
            let _ = store.bookmark_set_tags(b.id, &tags);
            let collection = collection.trim();
            let collection = if collection.is_empty() {
                None
            } else {
                store.collection_add(collection).ok()
            };
            let _ = store.bookmark_set_collection(b.id, collection);
        }
        self.close_dialog();
        self.bookmarks_load();
    }

    pub fn collection_new_ask(&mut self) {
        self.dialog_data = json!({});
        self.show_dialog("collection-new");
    }

    pub fn collection_create(&mut self, name: &str) {
        if name.trim().is_empty() {
            return;
        }
        let created = app_data::store().borrow().collection_add(name);
        if self.dialog == "collection-new" {
            self.close_dialog();
        }
        if let Ok(id) = created {
            self.bookmarks_load();
            self.bookmarks_show(&format!("c{id}"));
        }
    }

    pub fn collection_delete(&mut self, id: &str) {
        let Ok(id) = id.parse::<i64>() else { return };
        let _ = app_data::store().borrow().collection_delete(id);
        if self.bookmarks.filter == format!("c{id}") {
            self.bookmarks.filter = "all".into();
        }
        self.bookmarks_load();
    }

    // --- JSON for QML --------------------------------------------------------------------------

    fn bookmark_visible(&self, b: &Bookmark) -> bool {
        in_filter(&self.bookmarks.filter, b)
            && self
                .bookmarks
                .tag
                .as_ref()
                .is_none_or(|t| b.tags.contains(t))
    }

    fn filter_name(&self) -> String {
        match self.bookmarks.filter.as_str() {
            "favourites" => gettext("Favourites"),
            f => f
                .strip_prefix('c')
                .and_then(|c| c.parse::<i64>().ok())
                .and_then(|c| self.bookmarks.collections.iter().find(|x| x.id == c))
                .map(|c| c.name.clone())
                .unwrap_or_else(|| gettext("All Bookmarks")),
        }
    }

    /// The Bookmarks section of the sidebar.
    pub fn bookmarks_sidebar_json(&self) -> Value {
        let on = self.tools.page == "bookmarks";
        let row = |filter: &str, label: String, icon: &str, count: usize| {
            json!({ "id": format!("bookmarks:{filter}"), "label": label, "icon": icon,
                    "count": count, "selected": on && self.bookmarks.filter == filter,
                    "action": "win.bookmarks-show", "param": filter })
        };
        let all = &self.bookmarks.all;
        let mut rows = vec![
            row("all", gettext("All Bookmarks"), "bookmark", all.len()),
            row(
                "favourites",
                gettext("Favourites"),
                "star",
                all.iter().filter(|b| b.favourite).count(),
            ),
        ];
        for c in &self.bookmarks.collections {
            let count = all.iter().filter(|b| b.collection == Some(c.id)).count();
            let mut entry = row(&format!("c{}", c.id), c.name.clone(), "folder", count);
            entry["collection"] = json!(c.id);
            rows.push(entry);
        }
        json!(rows)
    }

    pub fn bookmarks_json(&self) -> Value {
        let view = settings::get().string("bookmark-view").to_string();
        let view = if VIEWS.contains(&view.as_str()) {
            view
        } else {
            "grid".into()
        };
        let shown: Vec<&Bookmark> = self
            .bookmarks
            .all
            .iter()
            .filter(|b| self.bookmark_visible(b))
            .collect();
        let mut tags: Vec<String> = self
            .bookmarks
            .all
            .iter()
            // Tags of the links in this filter, before the tag choice.
            .filter(|b| in_filter(&self.bookmarks.filter, b))
            .flat_map(|b| b.tags.iter().cloned())
            .collect();
        tags.sort();
        tags.dedup();
        let cards: Vec<Value> = shown
            .iter()
            .map(|b| {
                let collection = b
                    .collection
                    .and_then(|c| self.bookmarks.collections.iter().find(|x| x.id == c))
                    .map(|c| c.name.clone());
                json!({
                    "id": b.id,
                    "url": b.url,
                    "title": b.title.clone().unwrap_or_default(),
                    "site": b.site.clone().unwrap_or_else(|| bookmarks::site_of(&b.url)),
                    "kind": b.kind,
                    "image": b.image.as_ref().filter(|p| p.is_file())
                        .map(|p| format!("file://{}", p.display())).unwrap_or_default(),
                    "favourite": b.favourite,
                    "tags": b.tags,
                    "collection": collection.unwrap_or_default(),
                    "date": format::date(b.created),
                    "fetching": self.bookmarks.fetching.contains(&b.id),
                    // A stable colour for the placeholder picture.
                    "hue": hue(&b.url),
                })
            })
            .collect();
        let count = cards.len();
        json!({
            "title": self.filter_name(),
            "filter": self.bookmarks.filter,
            "collection": self.bookmarks.filter.strip_prefix('c').and_then(|c| c.parse::<i64>().ok()),
            "view": view,
            "tags": tags,
            "tag": self.bookmarks.tag,
            "cards": cards,
            "summary": format!("{count} {}", if count == 1 { gettext("link") } else { gettext("links") }),
            "fetch": settings::get().boolean("bookmark-fetch"),
        })
    }

    #[cfg(feature = "e2e")]
    pub fn bookmarks_test_json(&self) -> Value {
        json!({
            "filter": self.bookmarks.filter,
            "fetching": self.bookmarks.fetching.len(),
            "fetch": settings::get().boolean("bookmark-fetch"),
            "view": settings::get().string("bookmark-view").to_string(),
            "all": self.bookmarks.all.iter().map(|b| json!({
                "id": b.id, "url": b.url, "title": b.title, "site": b.site, "kind": b.kind,
                "image": b.image.as_ref().map(|p| p.display().to_string()),
                "image_exists": b.image.as_ref().is_some_and(|p| p.is_file()),
                "fetched": b.fetched, "favourite": b.favourite, "tags": b.tags, "collection": b.collection,
            })).collect::<Vec<_>>(),
        })
    }
}

/// "all", "favourites" or "c<id>".
fn in_filter(filter: &str, b: &Bookmark) -> bool {
    match filter {
        "favourites" => b.favourite,
        f => match f.strip_prefix('c').and_then(|c| c.parse::<i64>().ok()) {
            Some(c) => b.collection == Some(c),
            None => true,
        },
    }
}

fn hue(text: &str) -> u32 {
    text.bytes()
        .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32))
        % 360
}
