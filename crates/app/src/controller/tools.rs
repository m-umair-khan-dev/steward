//! The tool pages (Phase 5): Free Up Space, Installed and About This
//! Computer. One at a time covers the folder view, like a workspace page;
//! going to a folder or a workspace closes it.

use serde_json::{Value, json};

use super::{Controller, dirty};

#[derive(Default, Debug)]
pub struct ToolsState {
    /// "clean", "installed", "about", "agents", "markdown", or "" when no tool page is open.
    pub page: String,
}

pub const PAGES: [&str; 5] = ["clean", "installed", "about", "agents", "markdown"];

impl Controller {
    pub fn open_tool(&mut self, page: &str) {
        if !PAGES.contains(&page) || (page == "markdown" && self.md_name().is_empty()) {
            return;
        }
        self.workspace.open = None;
        self.close_search();
        self.tools.page = page.to_string();
        crate::actions::set_enabled("back", true);
        self.mark(dirty::TOOLS | dirty::SIDEBAR | dirty::WORKSPACE | dirty::NAV);
        match page {
            "clean" => self.clean_opened(),
            "installed" => self.installed_opened(),
            "agents" => self.agents_opened(),
            "markdown" => {}
            _ => self.about_opened(),
        }
    }

    pub fn close_tool(&mut self) {
        if !self.tools.page.is_empty() {
            self.tools.page.clear();
            crate::actions::set_enabled("back", !self.back.is_empty());
            self.mark(dirty::TOOLS | dirty::SIDEBAR | dirty::NAV);
        }
    }

    pub fn tools_json(&self) -> Value {
        let page = self.tools.page.as_str();
        json!({
            "page": page,
            "clean": if page == "clean" { self.clean_json() } else { Value::Null },
            "installed": if page == "installed" { self.installed_json() } else { Value::Null },
            "about": if page == "about" { self.about_json() } else { Value::Null },
            "agents": if page == "agents" { self.agents_json() } else { Value::Null },
            "markdown": if page == "markdown" { self.markdown_json() } else { Value::Null },
        })
    }
}
