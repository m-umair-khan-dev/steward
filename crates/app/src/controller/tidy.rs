//! Tidy (P4-T03): plan, show the plan, then run it as one batch.

use std::path::PathBuf;

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use cairn_core::ops::Request;
use cairn_core::plan::{self, Plan, SkipReason};

use super::{Controller, Event};
use crate::format;

#[derive(Default, Debug)]
pub struct TidyState {
    plan: Option<(PathBuf, Plan)>,
}

fn reason(reason: SkipReason) -> String {
    match reason {
        SkipReason::Downloading => gettext("still downloading"),
        SkipReason::Recent => gettext("added less than 2 minutes ago"),
        SkipReason::Hidden => gettext("hidden"),
    }
}

fn name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl Controller {
    /// Ctrl+Shift+O: plan tidying the folder on screen and show the plan.
    pub fn plan_tidy(&mut self) {
        let Some(folder) = self.current().and_then(|f| f.path()) else {
            return;
        };
        if self.in_trash() {
            return;
        }
        let plan = match plan::tidy(&folder, plan::now()) {
            Ok(plan) => plan,
            Err(error) => {
                self.emit(Event::Toast(
                    format!("{} {error}", gettext("Cannot tidy this folder:")),
                    false,
                ));
                return;
            }
        };
        let mut groups: Vec<Value> = Vec::new();
        for target in plan.folders() {
            let files: Vec<String> = plan
                .moves
                .iter()
                .filter(|m| m.to.parent() == Some(&target))
                .map(|m| name(&m.from))
                .collect();
            let kind = plan
                .moves
                .iter()
                .find(|m| m.to.parent() == Some(&target))
                .map(|m| m.kind.id())
                .unwrap_or("other");
            groups.push(json!({
                "folder": name(&target), "kind": kind, "count": files.len(),
                "new": plan.new_folders.contains(&target), "files": files,
            }));
        }
        let left: Vec<Value> = plan
            .skipped
            .iter()
            .map(|s| json!({ "name": name(&s.path), "reason": reason(s.reason) }))
            .collect();
        let folders = plan.folders().len() as u32;
        self.dialog_data = json!({
            "folder": name(&folder),
            "empty": plan.is_empty(),
            "summary": if plan.is_empty() {
                gettext("Nothing to tidy: every loose file is already in place.")
            } else {
                format!("{} {} {}. {}", format::files(plan.moves.len() as u32), gettext("will move into"),
                        format::folders(folders), gettext("Nothing is deleted, and Undo puts everything back."))
            },
            "groups": groups,
            "left": left,
        });
        self.tidy.plan = Some((folder, plan));
        self.show_dialog("tidy");
    }

    /// The dialog's Tidy button: runs only the plan that was shown.
    pub fn run_tidy(&mut self) {
        if self.dialog != "tidy" {
            return;
        }
        self.close_dialog();
        let Some((folder, plan)) = self.tidy.plan.take() else {
            return;
        };
        if plan.is_empty() {
            return;
        }
        let moves = plan
            .moves
            .iter()
            .map(|m| (m.from.clone(), m.to.clone()))
            .collect();
        let summary = format!("{} “{}”", gettext("Tidy"), name(&folder));
        self.submit_request(
            Request::Apply {
                kind: "tidy".into(),
                moves,
            },
            summary,
        );
    }

    pub fn cancel_tidy(&mut self) {
        self.tidy.plan = None;
        if self.dialog == "tidy" {
            self.close_dialog();
        }
    }

    #[cfg(feature = "e2e")]
    pub fn tidy_test_json(&self) -> Value {
        match &self.tidy.plan {
            Some((_, plan)) => json!({
                "moves": plan.moves.iter().map(|m| json!({
                    "from": m.from.display().to_string(), "to": m.to.display().to_string(),
                })).collect::<Vec<_>>(),
                "left": plan.skipped.iter().map(|s| json!({
                    "name": name(&s.path), "reason": format!("{:?}", s.reason),
                })).collect::<Vec<_>>(),
                "folders_moved": plan.moves.iter().filter(|m| m.from.is_dir()).count(),
            }),
            None => Value::Null,
        }
    }
}
