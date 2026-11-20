//! Installed (P5-T09): filled in with the package sources.

use serde_json::{Value, json};

use super::Controller;

#[derive(Default, Debug)]
pub struct InstalledState {}

impl Controller {
    pub fn installed_opened(&mut self) {}

    pub fn installed_count(&self) -> Option<usize> {
        None
    }

    pub fn installed_json(&self) -> Value {
        json!({})
    }
}
