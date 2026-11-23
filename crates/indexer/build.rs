//! Compiles the settings schema for development builds (as the app does), so
//! the indexer reads the same keys without installing anything.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let schema = manifest_dir.join("../../data/io.github.m_umair_khan_dev.steward.gschema.xml");
    println!("cargo:rerun-if-changed={}", schema.display());
    let schema_out = out_dir.join("schemas");
    fs::create_dir_all(&schema_out).expect("create OUT_DIR/schemas");
    fs::copy(
        &schema,
        schema_out.join("io.github.m_umair_khan_dev.steward.gschema.xml"),
    )
    .expect("copy schema");
    let status = Command::new("glib-compile-schemas")
        .arg("--strict")
        .arg(&schema_out)
        .status()
        .expect("run glib-compile-schemas");
    assert!(status.success(), "glib-compile-schemas failed");
}
