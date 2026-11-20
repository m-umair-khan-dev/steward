//! What is installed (DESIGN.md §4.2 "Installed", P5-T08): packages from
//! apt, Snap, Flatpak and the language tools in one list, each with why it
//! is there and its size. Read-only: removing is a command the user copies.
//!
//! Each tool is a [`PackageSource`]. Its parser is a pure function of the
//! tool's output, tested on recorded output in `tests/data/packages/`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::clean::tree_bytes;
use crate::tools::{Ran, Runner};

/// Why a package is there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// An app with a launcher.
    App,
    /// Installed on purpose.
    You,
    /// Pulled in by something else (a dependency, a runtime, part of a tool).
    CameWith,
}

impl Why {
    pub fn id(self) -> &'static str {
        match self {
            Why::App => "app",
            Why::You => "you",
            Why::CameWith => "came-with",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub tool: &'static str,
    pub name: String,
    pub version: String,
    pub summary: String,
    /// None when the tool does not say and it cannot be measured.
    pub bytes: Option<u64>,
    pub why: Why,
    /// When it was installed (seconds), where the tool records it.
    pub installed: Option<i64>,
    /// The command that would remove it, to copy. Never run.
    pub remove: String,
}

/// Where sources look on this machine (tests point these elsewhere).
#[derive(Debug, Clone)]
pub struct Places {
    pub home: PathBuf,
    pub dpkg_info: PathBuf,
    pub snaps: PathBuf,
    pub snap_desktop: PathBuf,
    /// `$CARGO_HOME` (else `~/.cargo`): installed binaries are in its `bin`.
    pub cargo_home: PathBuf,
    /// `$PIPX_HOME` (else `~/.local/share/pipx`, or the older `~/.local/pipx`).
    pub pipx_home: PathBuf,
}

impl Places {
    pub fn system(home: PathBuf) -> Self {
        Self {
            dpkg_info: "/var/lib/dpkg/info".into(),
            snaps: "/var/lib/snapd/snaps".into(),
            snap_desktop: "/var/lib/snapd/desktop/applications".into(),
            cargo_home: std::env::var_os("CARGO_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".cargo")),
            pipx_home: std::env::var_os("PIPX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    let old = home.join(".local/pipx");
                    if old.join("venvs").is_dir() {
                        old
                    } else {
                        home.join(".local/share/pipx")
                    }
                }),
            home,
        }
    }
}

pub trait PackageSource: Send + Sync {
    fn id(&self) -> &'static str;
    /// The tool's own name ("apt", "Snap", "npm" …).
    fn name(&self) -> &'static str;
    /// None when the tool is not installed (it is then not shown).
    fn list(&self, run: &dyn Runner, places: &Places) -> Option<Vec<Package>>;
}

/// One tool's packages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    pub id: &'static str,
    pub name: &'static str,
    pub packages: Vec<Package>,
}

impl Tool {
    pub fn bytes(&self) -> u64 {
        self.packages.iter().filter_map(|p| p.bytes).sum()
    }
}

pub fn sources() -> Vec<Box<dyn PackageSource>> {
    vec![
        Box::new(Apt),
        Box::new(Snap),
        Box::new(Flatpak),
        Box::new(Npm),
        Box::new(Pipx),
        Box::new(Pip),
        Box::new(Cargo),
        Box::new(Go),
        Box::new(Gem),
        Box::new(Brew),
    ]
}

/// Every installed tool that has packages, in [`sources`] order.
pub fn list_all(run: &dyn Runner, places: &Places) -> Vec<Tool> {
    sources()
        .iter()
        .filter_map(|s| {
            Some(Tool {
                id: s.id(),
                name: s.name(),
                packages: s.list(run, places).filter(|p| !p.is_empty())?,
            })
        })
        .collect()
}

/// Runs a tool; Err(()) means "not installed".
fn ask(run: &dyn Runner, program: &str, args: &[&str]) -> Result<Option<String>, ()> {
    match run.run(program, args) {
        Ran::Missing => Err(()),
        Ran::Failed(_) => Ok(None),
        Ran::Output(text) => Ok(Some(text)),
    }
}

fn measured(path: &Path) -> Option<u64> {
    path.symlink_metadata().ok().map(|_| tree_bytes(path))
}

// --- apt ---------------------------------------------------------------------------

pub const DPKG_FORMAT: &str =
    "-f=${Package}\t${Version}\t${Installed-Size}\t${db:Status-Status}\t${binary:Summary}\n";

/// Packages that own something in `/usr/share/applications`:
/// `dpkg-query -S /usr/share/applications` ("a, b:amd64, c: /usr/share/applications").
pub fn parse_dpkg_owners(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.rsplit_once(": ").map(|(owners, _)| owners))
        .flat_map(|owners| owners.split(", "))
        .map(|o| o.split(':').next().unwrap_or(o).trim().to_string())
        .filter(|o| !o.is_empty())
        .collect()
}

pub fn parse_dpkg(list: &str, manual: &str, apps: &[String]) -> Vec<Package> {
    let manual: Vec<&str> = manual
        .lines()
        .map(|l| l.split(':').next().unwrap_or(l).trim())
        .collect();
    let mut out: Vec<Package> = list
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.splitn(5, '\t').collect();
            let [name, version, size, status, summary] = fields.as_slice() else {
                return None;
            };
            if *status != "installed" {
                return None;
            }
            let why = if apps.iter().any(|a| a == name) {
                Why::App
            } else if manual.contains(name) {
                Why::You
            } else {
                Why::CameWith
            };
            Some(Package {
                tool: "apt",
                name: name.to_string(),
                version: version.to_string(),
                summary: summary.to_string(),
                bytes: size.trim().parse::<u64>().ok().map(|k| k * 1024),
                why,
                installed: None,
                remove: format!("sudo apt remove {name}"),
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.name == b.name);
    out
}

pub struct Apt;

impl PackageSource for Apt {
    fn id(&self) -> &'static str {
        "apt"
    }
    fn name(&self) -> &'static str {
        "apt"
    }
    fn list(&self, run: &dyn Runner, places: &Places) -> Option<Vec<Package>> {
        let list = ask(run, "dpkg-query", &["-W", DPKG_FORMAT]).ok()??;
        let manual = ask(run, "apt-mark", &["showmanual"])
            .ok()
            .flatten()
            .unwrap_or_default();
        let owners = ask(run, "dpkg-query", &["-S", "/usr/share/applications"])
            .ok()
            .flatten()
            .unwrap_or_default();
        let mut packages = parse_dpkg(&list, &manual, &parse_dpkg_owners(&owners));
        for p in &mut packages {
            p.installed = std::fs::metadata(places.dpkg_info.join(format!("{}.list", p.name)))
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);
        }
        Some(packages)
    }
}

// --- Snap --------------------------------------------------------------------------

/// `snap list`: (name, version, revision, notes).
pub fn parse_snap_list(text: &str) -> Vec<(String, String, String, String)> {
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    if !header.starts_with("Name") {
        return Vec::new();
    }
    lines
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            Some((
                f.first()?.to_string(),
                f.get(1)?.to_string(),
                f.get(2)?.to_string(),
                f.get(5).unwrap_or(&"-").to_string(),
            ))
        })
        .collect()
}

pub struct Snap;

impl PackageSource for Snap {
    fn id(&self) -> &'static str {
        "snap"
    }
    fn name(&self) -> &'static str {
        "Snap"
    }
    fn list(&self, run: &dyn Runner, places: &Places) -> Option<Vec<Package>> {
        let text = ask(run, "snap", &["list"]).ok()?.unwrap_or_default();
        let launchers: Vec<String> = std::fs::read_dir(&places.snap_desktop)
            .map(|e| {
                e.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        Some(
            parse_snap_list(&text)
                .into_iter()
                .map(|(name, version, rev, notes)| {
                    let system = notes
                        .split(',')
                        .any(|n| matches!(n, "base" | "core" | "snapd"));
                    let why = if system {
                        Why::CameWith
                    } else if launchers.iter().any(|l| l.starts_with(&format!("{name}_"))) {
                        Why::App
                    } else {
                        Why::You
                    };
                    Package {
                        tool: "snap",
                        bytes: std::fs::metadata(places.snaps.join(format!("{name}_{rev}.snap")))
                            .ok()
                            .map(|m| m.len()),
                        remove: format!("sudo snap remove {name}"),
                        summary: String::new(),
                        installed: None,
                        name,
                        version,
                        why,
                    }
                })
                .collect(),
        )
    }
}

// --- Flatpak -----------------------------------------------------------------------

/// `flatpak list --app|--runtime --columns=application,version,size,installation`
/// (tab-separated when not printing to a terminal).
pub fn parse_flatpak(text: &str, why: Why) -> Vec<Package> {
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').map(str::trim).collect();
            let id = f.first().filter(|s| !s.is_empty())?;
            let installation = f.get(3).copied().unwrap_or("system");
            Some(Package {
                tool: "flatpak",
                name: id.to_string(),
                version: f.get(1).copied().unwrap_or("").to_string(),
                summary: String::new(),
                bytes: f.get(2).and_then(|s| crate::tools::parse_size(s)),
                why,
                installed: None,
                remove: if installation == "user" {
                    format!("flatpak uninstall --user {id}")
                } else {
                    format!("flatpak uninstall {id}")
                },
            })
        })
        .collect()
}

pub struct Flatpak;

impl PackageSource for Flatpak {
    fn id(&self) -> &'static str {
        "flatpak"
    }
    fn name(&self) -> &'static str {
        "Flatpak"
    }
    fn list(&self, run: &dyn Runner, _: &Places) -> Option<Vec<Package>> {
        let columns = "--columns=application,version,size,installation";
        let apps = ask(run, "flatpak", &["list", "--app", columns]).ok()?;
        let runtimes = ask(run, "flatpak", &["list", "--runtime", columns]).ok()?;
        let mut out = parse_flatpak(&apps.unwrap_or_default(), Why::App);
        out.extend(parse_flatpak(&runtimes.unwrap_or_default(), Why::CameWith));
        Some(out)
    }
}

// --- npm ---------------------------------------------------------------------------

/// `npm ls -g --depth=0 --json`: (name, version).
pub fn parse_npm(text: &str) -> Vec<(String, String)> {
    let Ok(json) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = json["dependencies"]
        .as_object()
        .map(|deps| {
            deps.iter()
                .map(|(name, info)| {
                    (
                        name.clone(),
                        info["version"].as_str().unwrap_or("").to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

pub struct Npm;

impl PackageSource for Npm {
    fn id(&self) -> &'static str {
        "npm"
    }
    fn name(&self) -> &'static str {
        "npm"
    }
    fn list(&self, run: &dyn Runner, _: &Places) -> Option<Vec<Package>> {
        // npm writes a log of every run and checks for updates unless told not to.
        let quiet = ["--logs-max=0", "--no-update-notifier"];
        let text = ask(
            run,
            "npm",
            &["ls", "-g", "--depth=0", "--json", quiet[0], quiet[1]],
        )
        .ok()?;
        let root = ask(run, "npm", &["root", "-g", quiet[0], quiet[1]])
            .ok()
            .flatten()
            .map(|r| PathBuf::from(r.trim()));
        Some(
            parse_npm(&text.unwrap_or_default())
                .into_iter()
                .map(|(name, version)| Package {
                    tool: "npm",
                    bytes: root.as_ref().and_then(|r| measured(&r.join(&name))),
                    // npm and corepack come with Node.js itself.
                    why: if matches!(name.as_str(), "npm" | "corepack") {
                        Why::CameWith
                    } else {
                        Why::You
                    },
                    remove: format!("npm uninstall -g {name}"),
                    summary: String::new(),
                    installed: None,
                    name,
                    version,
                })
                .collect(),
        )
    }
}

// --- pipx and pip ------------------------------------------------------------------

/// `pipx list --json`: (name, version, venv folder).
pub fn parse_pipx(text: &str) -> Vec<(String, String, Option<PathBuf>)> {
    let Ok(json) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut out: Vec<_> = json["venvs"]
        .as_object()
        .map(|venvs| {
            venvs
                .iter()
                .map(|(name, venv)| {
                    let main = &venv["metadata"]["main_package"];
                    // An app path is <venv>/bin/<app>.
                    let folder = main["app_paths"]
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|p| p["__Path__"].as_str())
                        .and_then(|p| Path::new(p).parent()?.parent().map(Path::to_path_buf));
                    (
                        name.clone(),
                        main["package_version"].as_str().unwrap_or("").to_string(),
                        folder,
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

pub struct Pipx;

impl PackageSource for Pipx {
    fn id(&self) -> &'static str {
        "pipx"
    }
    fn name(&self) -> &'static str {
        "pipx"
    }
    /// Read from each venv's `pipx_metadata.json` (what `pipx list --json`
    /// prints for it): running pipx would write a log of the run.
    fn list(&self, run: &dyn Runner, places: &Places) -> Option<Vec<Package>> {
        if !run.has("pipx") {
            return None;
        }
        let mut venvs = serde_json::Map::new();
        for entry in std::fs::read_dir(places.pipx_home.join("venvs"))
            .ok()?
            .flatten()
        {
            let Ok(text) = std::fs::read_to_string(entry.path().join("pipx_metadata.json")) else {
                continue;
            };
            if let Ok(metadata) = serde_json::from_str::<Value>(&text) {
                venvs.insert(
                    entry.file_name().to_string_lossy().into_owned(),
                    serde_json::json!({ "metadata": metadata }),
                );
            }
        }
        let text = serde_json::json!({ "venvs": venvs }).to_string();
        Some(
            parse_pipx(&text)
                .into_iter()
                .map(|(name, version, folder)| Package {
                    tool: "pipx",
                    bytes: folder.as_deref().and_then(measured),
                    why: Why::You,
                    remove: format!("pipx uninstall {name}"),
                    summary: String::new(),
                    installed: None,
                    name,
                    version,
                })
                .collect(),
        )
    }
}

/// `pip list --format=json`: (name, version).
pub fn parse_pip(text: &str) -> Vec<(String, String)> {
    let Ok(Value::Array(list)) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = list
        .iter()
        .filter_map(|p| {
            Some((
                p["name"].as_str()?.to_string(),
                p["version"].as_str().unwrap_or("").to_string(),
            ))
        })
        .collect();
    out.sort();
    out
}

/// Packages installed with `pip install --user`.
pub struct Pip;

impl PackageSource for Pip {
    fn id(&self) -> &'static str {
        "pip"
    }
    fn name(&self) -> &'static str {
        "pip"
    }
    fn list(&self, run: &dyn Runner, _: &Places) -> Option<Vec<Package>> {
        let all = ask(
            run,
            "pip",
            &[
                "list",
                "--user",
                "--format=json",
                "--disable-pip-version-check",
            ],
        )
        .ok()?;
        let top: Vec<String> = ask(
            run,
            "pip",
            &[
                "list",
                "--user",
                "--not-required",
                "--format=json",
                "--disable-pip-version-check",
            ],
        )
        .ok()
        .flatten()
        .map(|t| parse_pip(&t).into_iter().map(|p| p.0).collect())
        .unwrap_or_default();
        Some(
            parse_pip(&all.unwrap_or_default())
                .into_iter()
                .map(|(name, version)| Package {
                    tool: "pip",
                    bytes: None,
                    why: if top.contains(&name) {
                        Why::You
                    } else {
                        Why::CameWith
                    },
                    remove: format!("pip uninstall {name}"),
                    summary: String::new(),
                    installed: None,
                    name,
                    version,
                })
                .collect(),
        )
    }
}

// --- Cargo and Go ------------------------------------------------------------------

/// `cargo install --list`: (name, version, binaries).
pub fn parse_cargo(text: &str) -> Vec<(String, String, Vec<String>)> {
    let mut out: Vec<(String, String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        if let Some(bin) = line.strip_prefix("    ") {
            if let Some(last) = out.last_mut() {
                last.2.push(bin.trim().to_string());
            }
        } else if let Some(head) = line.strip_suffix(':') {
            let mut parts = head.split_whitespace();
            let (Some(name), Some(version)) = (parts.next(), parts.next()) else {
                continue;
            };
            out.push((
                name.to_string(),
                version.trim_start_matches('v').to_string(),
                Vec::new(),
            ));
        }
    }
    out
}

pub struct Cargo;

impl PackageSource for Cargo {
    fn id(&self) -> &'static str {
        "cargo"
    }
    fn name(&self) -> &'static str {
        "Cargo"
    }
    fn list(&self, run: &dyn Runner, places: &Places) -> Option<Vec<Package>> {
        // `cargo install --list` creates its records when there are none:
        // only ask when something was installed.
        if !places.cargo_home.join(".crates.toml").is_file() {
            return None;
        }
        let text = ask(run, "cargo", &["install", "--list"]).ok()?;
        let bin = places.cargo_home.join("bin");
        Some(
            parse_cargo(&text.unwrap_or_default())
                .into_iter()
                .map(|(name, version, bins)| {
                    let sizes: Vec<Option<u64>> = bins
                        .iter()
                        .map(|b| std::fs::metadata(bin.join(b)).ok().map(|m| m.len()))
                        .collect();
                    Package {
                        tool: "cargo",
                        bytes: if bins.is_empty() {
                            None
                        } else {
                            sizes.into_iter().sum()
                        },
                        why: Why::You,
                        remove: format!("cargo uninstall {name}"),
                        summary: String::new(),
                        installed: None,
                        name,
                        version,
                    }
                })
                .collect(),
        )
    }
}

/// `go version -m <binary>`: (module path, version).
pub fn parse_go_version(text: &str) -> Option<(String, String)> {
    let mut path = None;
    for line in text.lines() {
        let f: Vec<&str> = line
            .split('\t')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        match f.as_slice() {
            ["path", p, ..] => path = Some(p.to_string()),
            ["mod", _, version, ..] => return Some((path?, version.to_string())),
            _ => {}
        }
    }
    path.map(|p| (p, String::new()))
}

pub struct Go;

impl PackageSource for Go {
    fn id(&self) -> &'static str {
        "go"
    }
    fn name(&self) -> &'static str {
        "Go"
    }
    fn list(&self, run: &dyn Runner, places: &Places) -> Option<Vec<Package>> {
        let gobin = ask(run, "go", &["env", "GOBIN", "GOPATH"]).ok()?;
        let folder = gobin
            .as_deref()
            .and_then(|t| {
                let mut lines = t.lines();
                let bin = lines.next().unwrap_or("").trim();
                let path = lines.next().unwrap_or("").trim();
                if !bin.is_empty() {
                    Some(PathBuf::from(bin))
                } else if !path.is_empty() {
                    Some(Path::new(path.split(':').next()?).join("bin"))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| places.home.join("go/bin"));
        let mut binaries: Vec<PathBuf> = std::fs::read_dir(&folder)
            .map(|e| e.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        binaries.sort();
        Some(
            binaries
                .into_iter()
                .filter_map(|bin| {
                    let text = ask(run, "go", &["version", "-m", bin.to_str()?])
                        .ok()
                        .flatten()?;
                    let (_, version) = parse_go_version(&text)?;
                    let name = bin.file_name()?.to_string_lossy().into_owned();
                    Some(Package {
                        tool: "go",
                        bytes: std::fs::metadata(&bin).ok().map(|m| m.len()),
                        why: Why::You,
                        remove: format!("rm {}", bin.display()),
                        summary: String::new(),
                        installed: None,
                        name,
                        version,
                    })
                })
                .collect(),
        )
    }
}

// --- RubyGems and Homebrew ---------------------------------------------------------

/// `gem list --local`: (name, newest version, only a default gem).
pub fn parse_gem(text: &str) -> Vec<(String, String, bool)> {
    text.lines()
        .filter_map(|line| {
            let (name, versions) = line.split_once(" (")?;
            let versions = versions.strip_suffix(')')?;
            let list: Vec<&str> = versions.split(", ").collect();
            let default_only = list.iter().all(|v| v.starts_with("default: "));
            let newest = list.first()?.trim_start_matches("default: ").to_string();
            Some((name.trim().to_string(), newest, default_only))
        })
        .collect()
}

pub struct Gem;

impl PackageSource for Gem {
    fn id(&self) -> &'static str {
        "gem"
    }
    fn name(&self) -> &'static str {
        "RubyGems"
    }
    fn list(&self, run: &dyn Runner, _: &Places) -> Option<Vec<Package>> {
        let text = ask(run, "gem", &["list", "--local"]).ok()?;
        Some(
            parse_gem(&text.unwrap_or_default())
                .into_iter()
                .map(|(name, version, default_only)| Package {
                    tool: "gem",
                    bytes: None,
                    // Default gems come with Ruby itself.
                    why: if default_only {
                        Why::CameWith
                    } else {
                        Why::You
                    },
                    remove: format!("gem uninstall {name}"),
                    summary: String::new(),
                    installed: None,
                    name,
                    version,
                })
                .collect(),
        )
    }
}

/// `brew list --versions`: (name, newest version).
pub fn parse_brew_versions(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let mut f = line.split_whitespace();
            Some((f.next()?.to_string(), f.last().unwrap_or("").to_string()))
        })
        .collect()
}

pub struct Brew;

impl PackageSource for Brew {
    fn id(&self) -> &'static str {
        "brew"
    }
    fn name(&self) -> &'static str {
        "Homebrew"
    }
    fn list(&self, run: &dyn Runner, _: &Places) -> Option<Vec<Package>> {
        let formulae = ask(run, "brew", &["list", "--formula", "--versions"]).ok()?;
        let leaves: Vec<String> = ask(run, "brew", &["leaves"])
            .ok()
            .flatten()
            .map(|t| t.lines().map(|l| l.trim().to_string()).collect())
            .unwrap_or_default();
        let casks = ask(run, "brew", &["list", "--cask", "--versions"])
            .ok()
            .flatten()
            .unwrap_or_default();
        let cellar = ask(run, "brew", &["--cellar"])
            .ok()
            .flatten()
            .map(|t| PathBuf::from(t.trim()));
        let mut out: Vec<Package> = parse_brew_versions(&formulae.unwrap_or_default())
            .into_iter()
            .map(|(name, version)| Package {
                tool: "brew",
                bytes: cellar.as_ref().and_then(|c| measured(&c.join(&name))),
                why: if leaves.contains(&name) {
                    Why::You
                } else {
                    Why::CameWith
                },
                remove: format!("brew uninstall {name}"),
                summary: String::new(),
                installed: None,
                name,
                version,
            })
            .collect();
        out.extend(
            parse_brew_versions(&casks)
                .into_iter()
                .map(|(name, version)| Package {
                    tool: "brew",
                    bytes: None,
                    why: Why::App,
                    remove: format!("brew uninstall --cask {name}"),
                    summary: String::new(),
                    installed: None,
                    name,
                    version,
                }),
        );
        Some(out)
    }
}

/// The list as Save List writes it: a header, then one tab-separated line
/// per package (tool, name, version, why, size in bytes or "-").
pub fn saved_list(tools: &[Tool]) -> String {
    let total: usize = tools.iter().map(|t| t.packages.len()).sum();
    let mut out = format!(
        "# Installed packages: {total} from {} tools\n# tool\tname\tversion\twhy\tbytes\n",
        tools.len()
    );
    for tool in tools {
        for p in &tool.packages {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                tool.id,
                p.name,
                p.version,
                p.why.id(),
                p.bytes.map_or("-".to_string(), |b| b.to_string())
            ));
        }
    }
    out
}
