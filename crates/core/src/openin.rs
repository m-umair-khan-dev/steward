//! Open In (P7-T02, P7-T03; DESIGN.md §4.1): the terminals and editors that
//! are installed, found from their desktop files, and the command that opens
//! a folder in each. Also the path bar's commands ("code .", "terminal").
//!
//! This module only works out *what* to run; the app starts it.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Terminal,
    Editor,
}

/// A terminal or editor Cairn knows how to open a folder in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    pub id: &'static str,
    pub name: &'static str,
    pub role: Role,
    /// Desktop file IDs that mean it is installed (any one).
    pub desktop_ids: &'static [&'static str],
    /// Arguments after the program; `{}` is the folder.
    pub args: &'static [&'static str],
    /// Words that run it from the path bar ("code", "zed").
    pub commands: &'static [&'static str],
}

pub const KNOWN: &[Known] = &[
    // Terminals (DESIGN.md §4.1).
    Known { id: "gnome-terminal", name: "Terminal", role: Role::Terminal,
            desktop_ids: &["org.gnome.Terminal.desktop"], args: &["--working-directory={}"], commands: &[] },
    Known { id: "ptyxis", name: "Ptyxis", role: Role::Terminal,
            desktop_ids: &["org.gnome.Ptyxis.desktop"], args: &["--new-window", "--working-directory={}"], commands: &["ptyxis"] },
    Known { id: "ghostty", name: "Ghostty", role: Role::Terminal,
            desktop_ids: &["com.mitchellh.ghostty.desktop"], args: &["--working-directory={}"], commands: &["ghostty"] },
    Known { id: "kitty", name: "Kitty", role: Role::Terminal,
            desktop_ids: &["kitty.desktop"], args: &["--directory", "{}"], commands: &["kitty"] },
    Known { id: "alacritty", name: "Alacritty", role: Role::Terminal,
            desktop_ids: &["Alacritty.desktop", "alacritty.desktop"], args: &["--working-directory", "{}"], commands: &["alacritty"] },
    Known { id: "wezterm", name: "WezTerm", role: Role::Terminal,
            desktop_ids: &["org.wezfurlong.wezterm.desktop"], args: &["start", "--cwd", "{}"], commands: &["wezterm"] },
    // Editors.
    Known { id: "vscode", name: "Visual Studio Code", role: Role::Editor,
            desktop_ids: &["code.desktop", "com.visualstudio.code.desktop"], args: &["{}"], commands: &["code"] },
    Known { id: "codium", name: "VSCodium", role: Role::Editor,
            desktop_ids: &["codium.desktop", "com.vscodium.codium.desktop"], args: &["{}"], commands: &["codium"] },
    Known { id: "cursor", name: "Cursor", role: Role::Editor,
            desktop_ids: &["cursor.desktop"], args: &["{}"], commands: &["cursor"] },
    Known { id: "zed", name: "Zed", role: Role::Editor,
            desktop_ids: &["dev.zed.Zed.desktop", "zed.desktop"], args: &["{}"], commands: &["zed"] },
    Known { id: "sublime", name: "Sublime Text", role: Role::Editor,
            desktop_ids: &["sublime_text.desktop", "com.sublimetext.three.desktop"], args: &["{}"], commands: &["subl"] },
    Known { id: "idea", name: "IntelliJ IDEA", role: Role::Editor,
            desktop_ids: &["jetbrains-idea.desktop", "jetbrains-idea-ce.desktop"], args: &["{}"], commands: &["idea"] },
    Known { id: "pycharm", name: "PyCharm", role: Role::Editor,
            desktop_ids: &["jetbrains-pycharm.desktop", "jetbrains-pycharm-ce.desktop"], args: &["{}"], commands: &["pycharm"] },
    Known { id: "rustrover", name: "RustRover", role: Role::Editor,
            desktop_ids: &["jetbrains-rustrover.desktop"], args: &["{}"], commands: &["rustrover"] },
    Known { id: "builder", name: "Builder", role: Role::Editor,
            desktop_ids: &["org.gnome.Builder.desktop"], args: &["--project={}"], commands: &["gnome-builder"] },
    Known { id: "text-editor", name: "Text Editor", role: Role::Editor,
            desktop_ids: &["org.gnome.TextEditor.desktop"], args: &["{}"], commands: &["gnome-text-editor"] },
];

/// An app found on this machine: what Known it is and its program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub known: &'static Known,
    /// The program from the desktop file's `Exec=` line.
    pub program: String,
    /// The desktop file's own name for it ("Visual Studio Code").
    pub label: String,
}

/// The program in a desktop file's `Exec=` value: the first word, without
/// quotes, `env VAR=…` prefixes or field codes.
pub fn program_of(exec: &str) -> Option<String> {
    let mut words = shell_words(exec).into_iter();
    let mut first = words.next()?;
    if first == "env" || first.ends_with("/env") {
        first = words.find(|w| !w.contains('='))?;
    }
    (!first.starts_with('%')).then_some(first)
}

fn shell_words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    for c in text.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, ' ' | '\t') => {
                if !word.is_empty() {
                    out.push(std::mem::take(&mut word));
                }
            }
            (_, c) => word.push(c),
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// Matches installed desktop files (id, Exec, Name) against what Cairn knows.
pub fn detect(desktop: &[(String, String, String)]) -> Vec<Found> {
    KNOWN
        .iter()
        .filter_map(|known| {
            let (_, exec, name) = desktop
                .iter()
                .find(|(id, _, _)| known.desktop_ids.contains(&id.as_str()))?;
            Some(Found {
                known,
                program: program_of(exec)?,
                label: name.clone(),
            })
        })
        .collect()
}

/// The command line that opens `folder`: the program and its arguments.
pub fn command(found: &Found, folder: &Path) -> Vec<String> {
    let dir = folder.display().to_string();
    std::iter::once(found.program.clone())
        .chain(found.known.args.iter().map(|a| a.replace("{}", &dir)))
        .collect()
}

/// A command the user wrote ("foot -D {}"): `{}` is the folder; without
/// `{}`, the folder is added at the end.
pub fn custom_command(template: &str, folder: &Path) -> Option<Vec<String>> {
    let dir = folder.display().to_string();
    let mut words = shell_words(template);
    if words.is_empty() {
        return None;
    }
    if words.iter().any(|w| w.contains("{}")) {
        for w in &mut words {
            *w = w.replace("{}", &dir);
        }
    } else {
        words.push(dir);
    }
    Some(words)
}

/// What a path-bar entry asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathCommand {
    /// Open the chosen terminal in this folder ("terminal", "term").
    Terminal(PathBuf),
    /// Open this app ("code .", "zed src").
    App(&'static Known, PathBuf),
}

/// Reads "code .", "zed", "zed src", "terminal" typed in the path bar, with
/// `here` the current folder. Anything else is not a command (it is a path).
pub fn path_command(text: &str, here: &Path) -> Option<PathCommand> {
    let words = shell_words(text.trim());
    let (first, rest) = words.split_first()?;
    let target = match rest {
        [] => here.to_path_buf(),
        [one] if one == "." => here.to_path_buf(),
        [one] => {
            let p = PathBuf::from(one.replace('~', &glib::home_dir().display().to_string()));
            if p.is_absolute() { p } else { here.join(p) }
        }
        _ => return None,
    };
    if matches!(first.as_str(), "terminal" | "term") {
        return Some(PathCommand::Terminal(target));
    }
    let known = KNOWN.iter().find(|k| k.commands.contains(&first.as_str()))?;
    Some(PathCommand::App(known, target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_from_exec_lines() {
        assert_eq!(program_of("/usr/share/code/code --unity-launch %F").as_deref(), Some("/usr/share/code/code"));
        assert_eq!(program_of("env BAMF_DESKTOP_FILE_HINT=x \"/opt/My App/app\" %U").as_deref(), Some("/opt/My App/app"));
        assert_eq!(program_of("%F"), None);
    }

    #[test]
    fn detection_and_commands() {
        let desktop = vec![
            ("org.gnome.Terminal.desktop".to_string(), "gnome-terminal".to_string(), "Terminal".to_string()),
            ("code.desktop".to_string(), "/usr/share/code/code %F".to_string(), "Visual Studio Code".to_string()),
            ("other.desktop".to_string(), "other".to_string(), "Other".to_string()),
        ];
        let found = detect(&desktop);
        assert_eq!(found.iter().map(|f| f.known.id).collect::<Vec<_>>(), ["gnome-terminal", "vscode"]);
        assert_eq!(command(&found[0], Path::new("/h/p")), ["gnome-terminal", "--working-directory=/h/p"]);
        assert_eq!(command(&found[1], Path::new("/h/p")), ["/usr/share/code/code", "/h/p"]);
        assert_eq!(custom_command("foot -D {}", Path::new("/h")).unwrap(), ["foot", "-D", "/h"]);
        assert_eq!(custom_command("my-term", Path::new("/h")).unwrap(), ["my-term", "/h"]);
    }

    #[test]
    fn path_bar_commands() {
        let here = Path::new("/h/proj");
        assert!(matches!(path_command("code .", here), Some(PathCommand::App(k, p)) if k.id == "vscode" && p == here));
        assert!(matches!(path_command("zed src", here), Some(PathCommand::App(k, p)) if k.id == "zed" && p == here.join("src")));
        assert_eq!(path_command("terminal", here), Some(PathCommand::Terminal(here.to_path_buf())));
        assert_eq!(path_command("~/Documents", here), None, "a path, not a command");
        assert_eq!(path_command("Invoices", here), None);
    }
}
