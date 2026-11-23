//! Activities search (P8-T04; DESIGN.md §7.3): a GNOME Shell search
//! provider (org.gnome.Shell.SearchProvider2) for workspaces, bookmark
//! collections and links, each with its own switch in Preferences.
//!
//! Result ids: "ws:ID", "col:ID", "link:ID". Activating a workspace or a
//! collection opens it in the window; a link opens in the browser. The
//! Shell finds the provider through
//! /usr/share/gnome-shell/search-providers/io.github.m_umair_khan_dev.steward.search-provider.ini
//! and starts Steward in the background through its D-Bus service file.

use gio::prelude::*;

use crate::{app_data, controller, settings};

pub const PATH: &str = "/io/github/m_umair_khan_dev/steward/SearchProvider";
const XML: &str = r#"
<node>
  <interface name="org.gnome.Shell.SearchProvider2">
    <method name="GetInitialResultSet">
      <arg type="as" name="terms" direction="in"/><arg type="as" name="results" direction="out"/>
    </method>
    <method name="GetSubsearchResultSet">
      <arg type="as" name="previous_results" direction="in"/><arg type="as" name="terms" direction="in"/>
      <arg type="as" name="results" direction="out"/>
    </method>
    <method name="GetResultMetas">
      <arg type="as" name="identifiers" direction="in"/><arg type="aa{sv}" name="metas" direction="out"/>
    </method>
    <method name="ActivateResult">
      <arg type="s" name="identifier" direction="in"/><arg type="as" name="terms" direction="in"/>
      <arg type="u" name="timestamp" direction="in"/>
    </method>
    <method name="LaunchSearch">
      <arg type="as" name="terms" direction="in"/><arg type="u" name="timestamp" direction="in"/>
    </method>
  </interface>
</node>"#;

/// Results per kind: the Shell shows only a few.
const PER_KIND: usize = 5;

/// One thing the provider can find.
struct Candidate {
    id: String,
    name: String,
    description: String,
    icon: &'static str,
    /// Text the terms are matched against.
    haystack: String,
}

/// Every term appears somewhere (case-insensitive).
fn matches(haystack: &str, terms: &[String]) -> bool {
    !terms.is_empty() && terms.iter().all(|t| haystack.contains(&t.to_lowercase()))
}

fn candidates() -> Vec<Candidate> {
    let s = settings::get();
    let store = app_data::store();
    let store = store.borrow();
    let mut out = Vec::new();
    if s.boolean("search-provider-workspaces") {
        for w in store.workspaces().unwrap_or_default() {
            out.push(Candidate {
                id: format!("ws:{}", w.id),
                haystack: format!("{} {}", w.name, w.description).to_lowercase(),
                description: if w.description.is_empty() {
                    gettextrs::gettext("Workspace")
                } else {
                    w.description.clone()
                },
                name: w.name,
                icon: "folder-symbolic",
            });
        }
    }
    if s.boolean("search-provider-collections") {
        for c in store.collections().unwrap_or_default() {
            out.push(Candidate {
                id: format!("col:{}", c.id),
                haystack: c.name.to_lowercase(),
                description: gettextrs::gettext("Bookmark collection"),
                name: c.name,
                icon: "user-bookmarks-symbolic",
            });
        }
    }
    if s.boolean("search-provider-links") {
        for b in store.bookmarks().unwrap_or_default() {
            let title = b.title.clone().unwrap_or_else(|| b.url.clone());
            out.push(Candidate {
                id: format!("link:{}", b.id),
                haystack: format!(
                    "{} {} {} {}",
                    title,
                    b.url,
                    b.site.clone().unwrap_or_default(),
                    b.tags.join(" ")
                )
                .to_lowercase(),
                description: b.url.clone(),
                name: title,
                icon: "web-browser-symbolic",
            });
        }
    }
    out
}

/// Ids matching `terms`, at most a few of each kind; `within` limits them
/// to an earlier result set (subsearch).
pub fn results(terms: &[String], within: Option<&[String]>) -> Vec<String> {
    let mut counts = std::collections::HashMap::new();
    candidates()
        .into_iter()
        .filter(|c| within.is_none_or(|w| w.contains(&c.id)))
        .filter(|c| matches(&c.haystack, terms))
        .filter(|c| {
            let kind = c.id.split(':').next().unwrap_or("").to_string();
            let n = counts.entry(kind).or_insert(0usize);
            *n += 1;
            *n <= PER_KIND
        })
        .map(|c| c.id)
        .collect()
}

fn metas(ids: &[String]) -> glib::Variant {
    let all = candidates();
    let dicts: Vec<glib::Variant> = ids
        .iter()
        .filter_map(|id| all.iter().find(|c| &c.id == id))
        .map(|c| {
            let dict = glib::VariantDict::new(None);
            dict.insert("id", c.id.as_str());
            dict.insert("name", c.name.as_str());
            dict.insert("description", c.description.as_str());
            // A serialised GIcon: a themed icon's name serialises as itself.
            let icon = IconExt::to_string(&gio::ThemedIcon::new(c.icon))
                .map(|s| s.to_string())
                .unwrap_or_else(|| c.icon.to_string());
            dict.insert("gicon", icon.as_str());
            dict.end()
        })
        .collect();
    glib::Variant::array_from_iter_with_type(glib::VariantTy::VARDICT, dicts)
}

/// Opens a result.
pub fn activate(id: &str) {
    let Some((kind, value)) = id.split_once(':') else {
        return;
    };
    controller::with(|c| match kind {
        "ws" => {
            if let Ok(id) = value.parse() {
                c.open_workspace(id);
                c.present();
            }
        }
        "col" => {
            c.bookmarks_show(&format!("c{value}"));
            c.present();
        }
        "link" => c.bookmark_open(value),
        _ => {}
    });
}

/// "Search in Steward": the window's own search, with the same words.
fn launch_search(terms: &[String]) {
    let query = terms.join(" ");
    controller::with(|c| {
        c.present();
        c.open_search();
        c.set_search_query(&query);
    });
}

/// Serves the provider on the app's bus connection.
pub fn register(connection: &gio::DBusConnection) {
    let info = gio::DBusNodeInfo::for_xml(XML)
        .expect("valid introspection XML")
        .lookup_interface("org.gnome.Shell.SearchProvider2")
        .expect("interface in XML");
    let result = connection
        .register_object(PATH, &info)
        .method_call(|_, _, _, _, method, params, invocation| match method {
            "GetInitialResultSet" => {
                let terms: Vec<String> = params.child_get(0);
                invocation.return_value(Some(&(results(&terms, None),).to_variant()));
            }
            "GetSubsearchResultSet" => {
                let previous: Vec<String> = params.child_get(0);
                let terms: Vec<String> = params.child_get(1);
                invocation.return_value(Some(&(results(&terms, Some(&previous)),).to_variant()));
            }
            "GetResultMetas" => {
                let ids: Vec<String> = params.child_get(0);
                invocation.return_value(Some(&glib::Variant::tuple_from_iter([metas(&ids)])));
            }
            "ActivateResult" => {
                let id: String = params.child_get(0);
                invocation.return_value(None);
                glib::idle_add_local_once(move || activate(&id));
            }
            "LaunchSearch" => {
                let terms: Vec<String> = params.child_get(0);
                invocation.return_value(None);
                glib::idle_add_local_once(move || launch_search(&terms));
            }
            _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method),
        })
        .build();
    if let Err(error) = result {
        glib::g_warning!("steward", "could not offer Activities search: {error}");
    }
}
