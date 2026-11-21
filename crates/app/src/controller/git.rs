//! The Git chip in the path bar (P7-T01; reference: developers.gif): the
//! branch of the folder's repository and its uncommitted changes, worked out
//! on a background thread whenever the folder changes or a file operation
//! finishes.

use std::path::PathBuf;

use gio::prelude::*;
use serde_json::{Value, json};

use cairn_core::git::{self, GitInfo};

use super::{Controller, dirty, later};

#[derive(Default, Debug)]
pub struct GitState {
    /// The folder the info is for, and the info (None: not a repository).
    shown: Option<(PathBuf, Option<GitInfo>)>,
    ticket: u64,
}

impl Controller {
    /// Looks again (after navigating, or after a file operation).
    pub fn refresh_git(&mut self) {
        let Some(folder) = self.current().and_then(|f| f.path()) else {
            self.git.shown = None;
            return;
        };
        self.git.ticket += 1;
        let ticket = self.git.ticket;
        std::thread::spawn(move || {
            let info = git::info(&folder);
            glib::MainContext::default().invoke(move || {
                later(move |c| {
                    if c.git.ticket == ticket {
                        c.git.shown = Some((folder, info));
                        c.mark(dirty::PATH);
                    }
                })
            });
        });
    }

    pub fn git_json(&self) -> Value {
        let here = self.current().and_then(|f| f.path());
        match &self.git.shown {
            Some((folder, Some(info))) if Some(folder) == here.as_ref() => json!({
                "branch": info.branch,
                "changes": info.changes,
                "root": info.root.display().to_string(),
            }),
            _ => Value::Null,
        }
    }
}
