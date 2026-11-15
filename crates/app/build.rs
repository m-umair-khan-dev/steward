//! Compiles the Blueprint layouts to GtkBuilder XML, then bundles them with the
//! stylesheet into a GResource that is embedded in the binary.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"));
    let data_dir = manifest_dir.join("../../data");
    let blueprint_dir = data_dir.join("ui");
    let ui_out = out_dir.join("ui");
    fs::create_dir_all(&ui_out).expect("create OUT_DIR/ui");

    let mut blueprints: Vec<PathBuf> = fs::read_dir(&blueprint_dir)
        .expect("read data/ui")
        .map(|entry| entry.expect("read data/ui entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "blp"))
        .collect();
    blueprints.sort();
    println!("cargo:rerun-if-changed={}", blueprint_dir.display());
    for blueprint in &blueprints {
        println!("cargo:rerun-if-changed={}", blueprint.display());
    }

    let status = Command::new("blueprint-compiler")
        .arg("batch-compile")
        .arg(&ui_out)
        .arg(&blueprint_dir)
        .args(&blueprints)
        .status()
        .expect("run blueprint-compiler (installed in the dev image)");
    assert!(status.success(), "blueprint-compiler failed");

    let gresource = data_dir.join("resources.gresource.xml");
    println!("cargo:rerun-if-changed={}", gresource.display());
    println!(
        "cargo:rerun-if-changed={}",
        data_dir.join("style.css").display()
    );
    glib_build_tools::compile_resources(
        &[out_dir.as_path(), data_dir.as_path()],
        gresource.to_str().expect("UTF-8 path"),
        "cairn.gresource",
    );
}
