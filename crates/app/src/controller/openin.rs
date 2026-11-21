//! Open In (P7-T02) and the path bar's commands (P7-T03; reference:
//! developers.gif): open the current folder in a terminal (F4) or an editor
//! (Shift+F4), chosen in Preferences from the apps installed, or your own
//! command.

use std::path::{Path, PathBuf};

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use cairn_core::openin::{self, Found, PathCommand, Role};

use super::{Controller, Event, dirty};
use crate::settings;

#[derive(Default, Debug)]
pub struct OpenInState {
    found: Option<Vec<Found>>,
    /// The last command started (tests read it): program and arguments, folder.
    last: Option<(Vec<String>, String)>,
}

fn on_path(program: &str) -> bool {
    cairn_core::tools::on_path(program)
}

impl Controller {
    fn openin_found(&mut self) -> &[Found] {
        if self.openin.found.is_none() {
            let desktop: Vec<(String, String, String)> = gio::AppInfo::all()
                .into_iter()
                .filter_map(|app| {
                    Some((
                        app.id()?.to_string(),
                        app.commandline()?.to_string_lossy().into_owned(),
                        app.display_name().to_string(),
                    ))
                })
                .collect();
            self.openin.found = Some(openin::detect(&desktop));
        }
        self.openin.found.as_deref().unwrap_or_default()
    }

    /// Looks for installed apps again (Preferences opens, an app was added).
    pub fn openin_rescan(&mut self) {
        self.openin.found = None;
        self.mark(dirty::PREFS);
    }

    /// The command that opens `folder` in the chosen app for `role`.
    fn openin_command(&mut self, role: Role, folder: &Path) -> Option<(Vec<String>, String)> {
        let s = settings::get();
        let (key, command_key) = match role {
            Role::Terminal => ("open-in-terminal", "open-in-terminal-command"),
            Role::Editor => ("open-in-editor", "open-in-editor-command"),
        };
        let chosen = s.string(key).to_string();
        if chosen == "custom" {
            let words = openin::custom_command(&s.string(command_key), folder)?;
            return Some((words, gettext("your command")));
        }
        let found: Vec<Found> = self.openin_found().iter().filter(|f| f.known.role == role).cloned().collect();
        if let Some(f) = found.iter().find(|f| f.known.id == chosen) {
            return Some((openin::command(f, folder), f.label.clone()));
        }
        // Nothing chosen: the system's terminal (it opens in the working
        // folder, which `launch` sets), else the first one found.
        if role == Role::Terminal && chosen.is_empty() {
            for program in ["xdg-terminal-exec", "x-terminal-emulator"] {
                if on_path(program) {
                    return Some((vec![program.to_string()], gettext("the terminal")));
                }
            }
        }
        let first = found.first()?;
        Some((openin::command(first, folder), first.label.clone()))
    }

    fn launch(&mut self, argv: Vec<String>, folder: &Path, label: &str) {
        let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
        launcher.set_cwd(folder);
        let args: Vec<&std::ffi::OsStr> = argv.iter().map(std::ffi::OsStr::new).collect();
        match launcher.spawn(&args) {
            Ok(_) => {
                self.openin.last = Some((argv, folder.display().to_string()));
                self.emit(Event::Toast(format!("{} {label}", gettext("Opened in")), false));
            }
            Err(error) => self.emit(Event::Toast(
                format!("{} {label}: {error}", gettext("Could not start")),
                false,
            )),
        }
    }

    fn openin_target(&self, path: &str) -> Option<PathBuf> {
        if path.is_empty() {
            self.current().and_then(|f| f.path())
        } else {
            Some(PathBuf::from(path))
        }
    }

    /// F4 / Shift+F4 and the menus: `path` empty means the current folder.
    pub fn open_in(&mut self, role: Role, path: &str) {
        let Some(folder) = self.openin_target(path) else { return };
        match self.openin_command(role, &folder) {
            Some((argv, label)) => self.launch(argv, &folder, &label),
            None => self.emit(Event::Toast(
                match role {
                    Role::Terminal => gettext("No terminal found. Choose one in Preferences › General."),
                    Role::Editor => gettext("No editor found. Choose one in Preferences › General."),
                },
                false,
            )),
        }
    }

    /// The path bar: "code .", "zed src", "terminal". True if it was a command.
    pub fn path_command(&mut self, text: &str) -> bool {
        let Some(here) = self.current().and_then(|f| f.path()) else {
            return false;
        };
        match openin::path_command(text, &here) {
            Some(PathCommand::Terminal(folder)) => {
                self.open_in(Role::Terminal, &folder.display().to_string());
                true
            }
            Some(PathCommand::App(known, folder)) => {
                let found = self.openin_found().iter().find(|f| f.known.id == known.id).cloned();
                match found {
                    Some(f) => self.launch(openin::command(&f, &folder), &folder, &f.label),
                    None => self.emit(Event::Toast(
                        format!("{} {}", known.name, gettext("is not installed")),
                        false,
                    )),
                }
                true
            }
            None => false,
        }
    }

    pub fn openin_json(&mut self) -> Value {
        let found: Vec<Found> = self.openin_found().to_vec();
        let list = |role: Role| -> Vec<Value> {
            found
                .iter()
                .filter(|f| f.known.role == role)
                .map(|f| json!({ "id": f.known.id, "label": f.label }))
                .collect()
        };
        let s = settings::get();
        json!({
            "terminals": list(Role::Terminal),
            "editors": list(Role::Editor),
            "terminal": s.string("open-in-terminal").as_str(),
            "editor": s.string("open-in-editor").as_str(),
            "terminalCommand": s.string("open-in-terminal-command").as_str(),
            "editorCommand": s.string("open-in-editor-command").as_str(),
            "systemTerminal": on_path("xdg-terminal-exec") || on_path("x-terminal-emulator"),
        })
    }

    #[cfg(feature = "e2e")]
    pub fn openin_test_json(&mut self) -> Value {
        let mut out = self.openin_json();
        out["last"] = match &self.openin.last {
            Some((argv, cwd)) => json!({ "argv": argv, "cwd": cwd }),
            None => Value::Null,
        };
        out
    }
}
