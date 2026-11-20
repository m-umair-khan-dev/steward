//! About This Computer (P5-T11): filled in with the system probes.

use serde_json::{Value, json};

use super::Controller;

#[derive(Default, Debug)]
pub struct AboutState {}

impl Controller {
    pub fn about_opened(&mut self) {}

    pub fn about_json(&self) -> Value {
        json!({})
    }
}
