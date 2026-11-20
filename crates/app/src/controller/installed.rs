//! Installed (P5-T09; reference: installed.png): packages from every tool
//! in one list, with tool chips, filters, why each package is there, sizes,
//! Save List, and the remove command to copy. Read-only.

use gettextrs::gettext;
use serde_json::{Value, json};

use cairn_core::packages::{self, Package, Places, Tool, Why};

use super::{Controller, Event, dirty, later};
use crate::format;

#[derive(Default, Debug)]
pub struct InstalledState {
    tools: Option<Vec<Tool>>,
    looking: bool,
    /// "all", "app", "you" or "came-with".
    filter: String,
    /// One tool's packages only ("" for every tool).
    tool: String,
    ticket: u64,
    /// The last remove command copied (tests read it).
    copied: Option<String>,
}

/// A tool's colour on the bar and its chip (a kind tint).
fn tool_color(id: &str) -> &'static str {
    match id {
        "apt" => "images",
        "snap" => "pdfs",
        "flatpak" => "documents",
        "npm" => "audio",
        "pipx" | "pip" => "code",
        "cargo" => "archives",
        "go" => "videos",
        "gem" => "installers",
        _ => "folders",
    }
}

fn why_label(why: Why) -> String {
    match why {
        Why::App => gettext("App"),
        Why::You => gettext("You installed"),
        Why::CameWith => gettext("Came with others"),
    }
}

impl Controller {
    pub fn installed_opened(&mut self) {
        if self.installed.tools.is_none() && !self.installed.looking {
            self.installed_refresh();
        }
    }

    /// Asks every tool again, on a background thread.
    pub fn installed_refresh(&mut self) {
        self.installed.looking = true;
        self.installed.ticket += 1;
        let ticket = self.installed.ticket;
        self.mark(dirty::TOOLS);
        std::thread::spawn(move || {
            let places = Places::system(glib::home_dir());
            let tools = packages::list_all(&cairn_core::tools::System, &places);
            glib::MainContext::default().invoke(move || {
                later(move |c| {
                    if c.installed.ticket == ticket {
                        c.installed.looking = false;
                        c.installed.tools = Some(tools);
                        c.mark(dirty::TOOLS | dirty::SIDEBAR);
                    }
                })
            });
        });
    }

    pub fn installed_count(&self) -> Option<usize> {
        self.installed
            .tools
            .as_ref()
            .map(|t| t.iter().map(|t| t.packages.len()).sum())
    }

    pub fn installed_filter(&mut self, filter: &str) {
        self.installed.filter = filter.to_string();
        self.mark(dirty::TOOLS);
    }

    pub fn installed_tool(&mut self, tool: &str) {
        self.installed.tool = if self.installed.tool == tool {
            String::new()
        } else {
            tool.to_string()
        };
        self.mark(dirty::TOOLS);
    }

    fn installed_shown(&self) -> Vec<&Package> {
        let filter = self.installed.filter.as_str();
        self.installed
            .tools
            .iter()
            .flatten()
            .filter(|t| self.installed.tool.is_empty() || t.id == self.installed.tool)
            .flat_map(|t| &t.packages)
            .filter(|p| match filter {
                "app" => p.why == Why::App,
                "you" => p.why == Why::You,
                "came-with" => p.why == Why::CameWith,
                _ => true,
            })
            .collect()
    }

    /// Copies a package's remove command ("tool:name"). Nothing runs.
    pub fn installed_copy(&mut self, what: &str) {
        let Some((tool, name)) = what.split_once(':') else {
            return;
        };
        let Some(command) = self
            .installed
            .tools
            .iter()
            .flatten()
            .filter(|t| t.id == tool)
            .flat_map(|t| &t.packages)
            .find(|p| p.name == name)
            .map(|p| p.remove.clone())
        else {
            return;
        };
        crate::bridge::qobject::clipboard_set(
            &vec!["text/plain;charset=utf-8".into(), "text/plain".into()],
            &vec![command.clone(), command.clone()],
        );
        self.installed.copied = Some(command);
        self.emit(Event::Toast(
            gettext("Copied the remove command. Paste it into a terminal to run it."),
            false,
        ));
    }

    /// Save List: writes the whole list (every tool, every package).
    pub fn installed_save(&mut self, path: &str) {
        let Some(tools) = &self.installed.tools else {
            return;
        };
        let path = crate::paths::expand(path.trim_start_matches("file://"), None);
        let text = packages::saved_list(tools);
        let message = match cairn_core::ops::save_text(&path, &text) {
            Ok(()) => format!(
                "{} “{}”",
                gettext("Saved the list as"),
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            ),
            Err(error) => format!("{}: {error}", gettext("Could not save the list")),
        };
        self.emit(Event::Toast(message, false));
    }

    pub fn installed_json(&self) -> Value {
        let Some(tools) = &self.installed.tools else {
            return json!({ "looking": true, "tools": [], "filters": [], "packages": [] });
        };
        let all: Vec<&Package> = tools.iter().flat_map(|t| &t.packages).collect();
        let total_bytes: u64 = tools.iter().map(Tool::bytes).sum();
        let count = |why: Option<Why>| {
            all.iter()
                .filter(|p| why.is_none_or(|w| p.why == w))
                .count()
        };
        let filter = if self.installed.filter.is_empty() {
            "all"
        } else {
            &self.installed.filter
        };
        let names: std::collections::HashMap<&str, &str> =
            tools.iter().map(|t| (t.id, t.name)).collect();
        json!({
            "looking": self.installed.looking,
            "title": format!("{} {}", all.len(), gettext("packages")),
            "summary": format!("{} {} {} {}", format::size(total_bytes), gettext("on this computer, from"),
                               tools.len(), if tools.len() == 1 { gettext("tool") } else { gettext("tools") }),
            "tools": tools.iter().map(|t| json!({
                "id": t.id, "name": t.name, "count": t.packages.len(), "bytesText": format::size(t.bytes()),
                "share": if total_bytes > 0 { t.bytes() as f64 / total_bytes as f64 } else { 1.0 / tools.len() as f64 },
                "color": tool_color(t.id), "selected": self.installed.tool == t.id,
            })).collect::<Vec<_>>(),
            "filters": [
                { "id": "all", "label": gettext("All"), "count": count(None), "selected": filter == "all" },
                { "id": "app", "label": gettext("Apps"), "count": count(Some(Why::App)), "selected": filter == "app" },
                { "id": "you", "label": gettext("You installed"), "count": count(Some(Why::You)), "selected": filter == "you" },
                { "id": "came-with", "label": gettext("Came with others"), "count": count(Some(Why::CameWith)),
                  "selected": filter == "came-with" },
            ],
            "packages": self.installed_shown().iter().map(|p| json!({
                "key": format!("{}:{}", p.tool, p.name),
                "tool": p.tool, "toolName": names.get(p.tool).copied().unwrap_or(p.tool),
                "color": tool_color(p.tool),
                "name": p.name, "version": p.version, "summary": p.summary,
                "why": p.why.id(), "whyLabel": why_label(p.why),
                "sizeText": p.bytes.map(format::size).unwrap_or_else(|| "—".into()),
                "date": p.installed.map(format::date).unwrap_or_default(),
                "remove": p.remove,
            })).collect::<Vec<_>>(),
        })
    }

    #[cfg(feature = "e2e")]
    pub fn installed_test_json(&self) -> Value {
        json!({
            "looking": self.installed.looking,
            "loaded": self.installed.tools.is_some(),
            "tools": self.installed.tools.as_ref().map(|tools| tools.iter().map(|t| (t.id.to_string(), json!({
                "count": t.packages.len(), "bytes": t.bytes(),
            }))).collect::<serde_json::Map<_, _>>()),
            "filter": if self.installed.filter.is_empty() { "all" } else { &self.installed.filter },
            "tool": self.installed.tool,
            "shown": self.installed_shown().iter().map(|p| format!("{}:{}", p.tool, p.name)).collect::<Vec<_>>(),
            "copied": self.installed.copied,
        })
    }
}
