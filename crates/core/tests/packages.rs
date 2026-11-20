//! L1 for `core::packages` (P5-T08): every parser on recorded output in
//! tests/data/packages, and the whole list against the golden Save List.

use std::path::{Path, PathBuf};

use cairn_core::packages::{self, Places, Why};
use cairn_core::tools::{Ran, Runner};

fn data(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/data/packages")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Answers as the tools recorded in tests/data/packages would.
struct Recorded;

impl Runner for Recorded {
    fn run(&self, program: &str, args: &[&str]) -> Ran {
        let file = match (program, args) {
            ("dpkg-query", ["-W", f]) if *f == packages::DPKG_FORMAT => "dpkg-query-list.txt",
            ("dpkg-query", ["-S", ..]) => "dpkg-query-owners.txt",
            ("apt-mark", ["showmanual"]) => "apt-mark-showmanual.txt",
            ("snap", ["list"]) => "snap-list.txt",
            ("flatpak", ["list", "--app", ..]) => "flatpak-apps.txt",
            ("flatpak", ["list", "--runtime", ..]) => "flatpak-runtimes.txt",
            ("npm", ["ls", ..]) => "npm-ls.json",
            ("npm", ["root", "-g", ..]) => {
                return Ran::Output("/nonexistent/lib/node_modules\n".into());
            }
            ("pip", ["list", "--user", "--not-required", ..]) => "pip-list-not-required.json",
            ("pip", ["list", "--user", ..]) => "pip-list.json",
            ("cargo", ["install", "--list"]) => "cargo-install-list.txt",
            ("gem", ["list", "--local"]) => "gem-list.txt",
            _ => return Ran::Missing,
        };
        Ran::Output(data(file))
    }
    fn has(&self, program: &str) -> bool {
        program == "pipx"
    }
}

/// A Cargo home with install records, and a pipx home with the recorded
/// venvs' metadata (what `pipx list --json` prints for each).
fn homes(name: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("cairn-packages-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let cargo = root.join("cargo");
    std::fs::create_dir_all(&cargo).unwrap();
    std::fs::write(cargo.join(".crates.toml"), "[v1]\n").unwrap();
    let pipx = root.join("pipx");
    let list: serde_json::Value = serde_json::from_str(&data("pipx-list.json")).unwrap();
    for (venv, info) in list["venvs"].as_object().unwrap() {
        let dir = pipx.join("venvs").join(venv);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pipx_metadata.json"), info["metadata"].to_string()).unwrap();
    }
    (cargo, pipx)
}

fn places(snap_desktop: PathBuf) -> Places {
    let (cargo_home, pipx_home) = homes(&snap_desktop.to_string_lossy().replace('/', "-"));
    Places {
        home: "/nonexistent/home".into(),
        snaps: "/nonexistent/snaps".into(),
        snap_desktop,
        cargo_home,
        pipx_home,
    }
}

#[test]
fn the_whole_list_equals_the_golden_file() {
    let tools = packages::list_all(&Recorded, &places("/nonexistent/desktop".into()));
    assert_eq!(
        packages::saved_list(&tools),
        data("installed-list.golden.txt")
    );
}

#[test]
fn tools_that_are_missing_are_not_shown() {
    let tools = packages::list_all(&Recorded, &places("/nonexistent/desktop".into()));
    let ids: Vec<&str> = tools.iter().map(|t| t.id).collect();
    assert!(!ids.contains(&"go") && !ids.contains(&"brew"));
    let expected: serde_json::Value = serde_json::from_str(&data("expected.json")).unwrap();
    for tool in &tools {
        assert_eq!(
            tool.packages.len() as u64,
            expected["tools"][tool.id]["count"].as_u64().unwrap(),
            "{}",
            tool.id
        );
        assert_eq!(
            tool.bytes(),
            expected["tools"][tool.id]["bytes"].as_u64().unwrap(),
            "{}",
            tool.id
        );
    }
}

#[test]
fn dpkg_reads_why_from_apt_mark_and_launchers() {
    let apps = packages::parse_dpkg_owners(&data("dpkg-query-owners.txt"));
    assert_eq!(apps, ["gimp", "vlc", "desktop-file-utils"]);
    let list = packages::parse_dpkg(
        &data("dpkg-query-list.txt"),
        &data("apt-mark-showmanual.txt"),
        &apps,
    );
    let why = |name: &str| list.iter().find(|p| p.name == name).unwrap().why;
    assert_eq!(why("gimp"), Why::App);
    assert_eq!(why("curl"), Why::You);
    assert_eq!(why("libc6"), Why::CameWith);
    assert!(
        list.iter().all(|p| p.name != "old-tool"),
        "only installed packages"
    );
    assert_eq!(
        list.iter().find(|p| p.name == "git").unwrap().remove,
        "sudo apt remove git"
    );
    assert_eq!(
        packages::parse_dpkg_owners("libfoo:amd64, bar: /usr/share/applications\n"),
        ["libfoo", "bar"]
    );
}

#[test]
fn a_snap_with_a_launcher_is_an_app() {
    let desktop = std::env::temp_dir().join(format!("cairn-snap-desktop-{}", std::process::id()));
    std::fs::create_dir_all(&desktop).unwrap();
    std::fs::write(desktop.join("firefox_firefox.desktop"), "[Desktop Entry]\n").unwrap();
    let tools = packages::list_all(&Recorded, &places(desktop.clone()));
    let snap = tools.iter().find(|t| t.id == "snap").unwrap();
    let why = |name: &str| snap.packages.iter().find(|p| p.name == name).unwrap().why;
    assert_eq!(why("firefox"), Why::App);
    assert_eq!(why("core24"), Why::CameWith);
    assert_eq!(why("spotless-notes"), Why::You);
    let _ = std::fs::remove_dir_all(desktop);
}

#[test]
fn flatpak_sizes_and_user_installs() {
    let apps = packages::parse_flatpak(&data("flatpak-apps.txt"), Why::App);
    assert_eq!(apps[0].bytes, Some(180_300_000));
    assert_eq!(apps[1].remove, "flatpak uninstall --user org.example.Notes");
}

#[test]
fn language_tools() {
    assert_eq!(packages::parse_npm(&data("npm-ls.json")).len(), 4);
    let pipx = packages::parse_pipx(&data("pipx-list.json"));
    assert_eq!(pipx[0].0, "black");
    assert_eq!(
        pipx[0].2.as_deref(),
        Some(Path::new("/nonexistent/pipx/venvs/black"))
    );
    assert_eq!(packages::parse_pip(&data("pip-list.json")).len(), 3);
    let cargo = packages::parse_cargo(&data("cargo-install-list.txt"));
    assert_eq!(
        cargo[1],
        ("ripgrep".into(), "14.1.1".into(), vec!["rg".to_string()])
    );
    let gems = packages::parse_gem(&data("gem-list.txt"));
    assert_eq!(gems[0], ("bundler".into(), "2.6.9".into(), false));
    assert_eq!(gems[1], ("json".into(), "2.10.2".into(), true));
    assert_eq!(
        packages::parse_brew_versions(&data("brew-list-formula.txt"))[0],
        ("fd".into(), "10.3.0".into())
    );
    assert_eq!(
        packages::parse_go_version(&data("go-version-m.txt")),
        Some(("golang.org/x/tools/gopls".into(), "v0.20.0".into()))
    );
    assert_eq!(
        packages::parse_snap_list("No snaps are installed yet.\n"),
        vec![]
    );
}
