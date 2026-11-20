//! About This Computer (DESIGN.md §4.6, P5-T10): the machine's facts, read
//! with what any user may read (`/sys`, `/proc`, `/etc`, the session bus).
//! Nothing here needs root; a fact that cannot be read is None and shows as
//! "Unknown".
//!
//! [`RealProbe`] reads this machine; [`FixtureProbe`] reads a JSON file, so
//! tests see fixed values.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::tools::{Ran, Runner};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Battery {
    /// Charge now, percent.
    pub charge: f64,
    /// "charging", "discharging", "full" or "not charging".
    pub state: String,
    /// Full charge now and as designed (Wh).
    pub full_wh: Option<f64>,
    pub design_wh: Option<f64>,
    pub cycles: Option<u32>,
    /// On mains power.
    pub plugged_in: bool,
    /// Draw or charge rate (W).
    pub power_w: Option<f64>,
}

impl Battery {
    /// Maximum capacity: full charge now as a share of the design (percent).
    pub fn capacity(&self) -> Option<f64> {
        match (self.full_wh, self.design_wh) {
            (Some(full), Some(design)) if design > 0.0 => Some((full / design * 100.0).min(100.0)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Memory {
    pub total: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    /// Share of the last 10 seconds some task waited for memory (percent).
    pub pressure: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Disk {
    pub size: u64,
    pub free: u64,
    pub filesystem: String,
    /// The drive's model, when the kernel knows it.
    pub drive: Option<String>,
    pub encrypted: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Display {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh: f64,
    pub scale: f64,
    pub builtin: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Security {
    pub disk_encryption: Option<bool>,
    pub firewall: Option<bool>,
    pub secure_boot: Option<bool>,
    pub apparmor: Option<bool>,
    pub auto_updates: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageCounts {
    pub apt: Option<u64>,
    pub snap: Option<u64>,
    pub flatpak: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SysInfo {
    pub host_name: String,
    pub user_name: String,
    /// "ThinkPad X1 Carbon Gen 13" (vendor and product, from DMI).
    pub model: Option<String>,
    pub vendor: Option<String>,
    pub laptop: bool,
    pub cpu: Option<String>,
    pub cores: Option<u32>,
    pub threads: Option<u32>,
    pub architecture: String,
    pub os: Option<String>,
    pub kernel: String,
    /// Seconds since boot.
    pub uptime: Option<u64>,
    pub shell: Option<String>,
    pub time_zone: Option<String>,
    pub language: Option<String>,
    /// When the system was set up (seconds): an estimate.
    pub in_use_since: Option<i64>,
    pub battery: Option<Battery>,
    pub memory: Option<Memory>,
    /// The hottest sensor, °C.
    pub temperature: Option<f64>,
    pub disk: Option<Disk>,
    pub displays: Vec<Display>,
    pub security: Security,
    pub packages: PackageCounts,
}

pub trait SysProbe: Send + Sync {
    fn probe(&self) -> SysInfo;
}

// --- reading this machine ----------------------------------------------------------

fn read(path: impl AsRef<Path>) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn read_num(path: impl AsRef<Path>) -> Option<f64> {
    read(path)?.parse().ok()
}

/// `KEY=value` files (/etc/os-release, ufw.conf).
fn key_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
    })
}

/// "Lenovo" + "21NS0000" + "ThinkPad X1 Carbon Gen 13" → the friendliest.
fn model(dmi: &Path) -> (Option<String>, Option<String>) {
    let junk = |s: &String| {
        let l = s.to_lowercase();
        l.contains("to be filled") || l.contains("default string") || l == "system product name"
    };
    let vendor = read(dmi.join("sys_vendor")).filter(|s| !junk(s));
    let product = read(dmi.join("product_name")).filter(|s| !junk(s));
    let version = read(dmi.join("product_version")).filter(|s| !junk(s));
    // Lenovo keeps the marketing name in product_version.
    let name = match (&vendor, &product, &version) {
        (Some(v), _, Some(ver))
            if v.eq_ignore_ascii_case("lenovo") && ver.starts_with("ThinkPad") =>
        {
            Some(ver.clone())
        }
        (Some(v), Some(p), _) if !p.starts_with(v.as_str()) => Some(format!("{v} {p}")),
        (_, Some(p), _) => Some(p.clone()),
        _ => None,
    };
    (name, vendor)
}

/// (model name, cores, threads) from /proc/cpuinfo.
pub fn parse_cpuinfo(text: &str) -> (Option<String>, Option<u32>, Option<u32>) {
    let mut name = None;
    let mut threads = 0;
    let mut cores = std::collections::HashSet::new();
    let (mut physical, mut core) = (String::new(), String::new());
    for line in text.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "processor" => threads += 1,
            "model name" | "Model" if name.is_none() => {
                name = Some(v.split_whitespace().collect::<Vec<_>>().join(" "))
            }
            "physical id" => physical = v.to_string(),
            "core id" => {
                core = v.to_string();
                cores.insert((physical.clone(), core.clone()));
            }
            _ => {}
        }
    }
    let _ = core;
    let threads = (threads > 0).then_some(threads);
    let cores = if cores.is_empty() {
        threads
    } else {
        Some(cores.len() as u32)
    };
    (name, cores, threads)
}

/// (total, available, swap total, swap free) from /proc/meminfo, in bytes.
pub fn parse_meminfo(text: &str) -> Option<(u64, u64, u64, u64)> {
    let get = |key: &str| -> Option<u64> {
        text.lines().find_map(|l| {
            let rest = l.strip_prefix(key)?.strip_prefix(':')?;
            Some(rest.split_whitespace().next()?.parse::<u64>().ok()? * 1024)
        })
    };
    Some((
        get("MemTotal")?,
        get("MemAvailable")?,
        get("SwapTotal").unwrap_or(0),
        get("SwapFree").unwrap_or(0),
    ))
}

/// "some avg10=1.23 …" from /proc/pressure/memory.
pub fn parse_pressure(text: &str) -> Option<f64> {
    text.lines()
        .find(|l| l.starts_with("some"))?
        .split_whitespace()
        .find_map(|f| f.strip_prefix("avg10="))?
        .parse()
        .ok()
}

fn battery(power: &Path) -> Option<Battery> {
    let entries: Vec<PathBuf> = std::fs::read_dir(power)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    let bat = entries.iter().find(|p| {
        read(p.join("type")).as_deref() == Some("Battery")
            && read(p.join("scope")).as_deref() != Some("Device")
    })?;
    let plugged_in = entries
        .iter()
        .filter(|p| read(p.join("type")).as_deref() == Some("Mains"))
        .any(|p| read(p.join("online")).as_deref() == Some("1"));
    // Energy (µWh) or charge (µAh, times the voltage) — batteries report one.
    let volts = read_num(bat.join("voltage_min_design")).map(|v| v / 1e6);
    let wh = |energy: &str, charge: &str| {
        read_num(bat.join(energy))
            .map(|e| e / 1e6)
            .or_else(|| Some(read_num(bat.join(charge))? / 1e6 * volts?))
    };
    Some(Battery {
        charge: read_num(bat.join("capacity"))?,
        state: read(bat.join("status")).unwrap_or_default().to_lowercase(),
        full_wh: wh("energy_full", "charge_full"),
        design_wh: wh("energy_full_design", "charge_full_design"),
        cycles: read_num(bat.join("cycle_count"))
            .map(|c| c as u32)
            .filter(|c| *c > 0),
        plugged_in,
        power_w: read_num(bat.join("power_now"))
            .map(|p| p / 1e6)
            .filter(|p| *p > 0.0),
    })
}

/// The mount info line of `/`: (major:minor, filesystem, source).
fn root_mount() -> Option<(String, String, String)> {
    let text = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    text.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(' ').collect();
        if fields.get(4) != Some(&"/") {
            return None;
        }
        let dash = fields.iter().position(|f| *f == "-")?;
        Some((
            fields[2].to_string(),
            fields.get(dash + 1)?.to_string(),
            fields.get(dash + 2)?.to_string(),
        ))
    })
}

/// True if the block device (or anything under it) is a LUKS container.
fn encrypted(block: &Path, depth: u32) -> bool {
    if depth > 6 {
        return false;
    }
    if read(block.join("dm/uuid")).is_some_and(|u| u.starts_with("CRYPT-")) {
        return true;
    }
    std::fs::read_dir(block.join("slaves"))
        .map(|e| e.flatten().any(|s| encrypted(&s.path(), depth + 1)))
        .unwrap_or(false)
}

fn drive_model(block: &Path, depth: u32) -> Option<String> {
    if depth > 6 {
        return None;
    }
    let real = block.canonicalize().ok()?;
    // A partition's parent is the disk.
    let disk = if real.join("partition").exists() {
        real.parent()?.to_path_buf()
    } else {
        real
    };
    read(disk.join("device/model")).or_else(|| {
        std::fs::read_dir(disk.join("slaves"))
            .ok()?
            .flatten()
            .find_map(|s| drive_model(&s.path(), depth + 1))
    })
}

fn disk() -> Option<Disk> {
    let stat = rustix::fs::statvfs("/").ok()?;
    let (device, filesystem, _) = root_mount().unwrap_or_default();
    let block = Path::new("/sys/dev/block").join(&device);
    let known = !device.is_empty() && block.exists();
    Some(Disk {
        size: stat.f_blocks * stat.f_frsize,
        free: stat.f_bavail * stat.f_frsize,
        filesystem,
        drive: if known { drive_model(&block, 0) } else { None },
        encrypted: known.then(|| encrypted(&block.canonicalize().unwrap_or(block.clone()), 0)),
    })
}

fn temperature() -> Option<f64> {
    std::fs::read_dir("/sys/class/thermal")
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("thermal_zone"))
        .filter_map(|e| read_num(e.path().join("temp")))
        .map(|milli| milli / 1000.0)
        .filter(|c| *c > 0.0 && *c < 150.0)
        .reduce(f64::max)
}

/// When the system was set up: the birth time of `/`, else the installer's
/// log folder.
fn in_use_since() -> Option<i64> {
    use rustix::fs::{AtFlags, CWD, StatxFlags, statx};
    if let Ok(st) = statx(CWD, "/", AtFlags::empty(), StatxFlags::BTIME)
        && st.stx_mask & StatxFlags::BTIME.bits() != 0
        && st.stx_btime.tv_sec > 0
    {
        return Some(st.stx_btime.tv_sec);
    }
    std::fs::metadata("/var/log/installer")
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

fn security(disk: Option<&Disk>) -> Security {
    let efi = Path::new("/sys/firmware/efi");
    let secure_boot = if efi.exists() {
        std::fs::read("/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c")
            .ok()
            .and_then(|b| b.last().map(|v| *v == 1))
    } else {
        Some(false)
    };
    let auto_updates = read("/etc/apt/apt.conf.d/20auto-upgrades").map(|t| {
        t.lines()
            .any(|l| l.contains("Unattended-Upgrade") && l.contains("\"1\""))
    });
    Security {
        disk_encryption: disk.and_then(|d| d.encrypted),
        firewall: read("/etc/ufw/ufw.conf")
            .and_then(|t| key_value(&t, "ENABLED"))
            .map(|v| v == "yes"),
        secure_boot,
        apparmor: read("/sys/module/apparmor/parameters/enabled").map(|v| v == "Y"),
        auto_updates,
    }
}

fn package_counts(run: &dyn Runner) -> PackageCounts {
    let lines = |program: &str, args: &[&str], skip: usize| -> Option<u64> {
        match run.run(program, args) {
            Ran::Output(t) => Some(
                t.lines()
                    .filter(|l| !l.trim().is_empty())
                    .count()
                    .saturating_sub(skip) as u64,
            ),
            _ => None,
        }
    };
    PackageCounts {
        apt: match run.run("dpkg-query", &["-W", "-f=${db:Status-Status}\n"]) {
            Ran::Output(t) => Some(t.lines().filter(|l| *l == "installed").count() as u64),
            _ => None,
        },
        snap: lines("snap", &["list"], 1),
        flatpak: lines("flatpak", &["list", "--app", "--columns=application"], 0),
    }
}

/// The value of `key` in an `a{sv}` dictionary.
fn lookup(dict: &glib::Variant, key: &str) -> Option<glib::Variant> {
    (0..dict.n_children())
        .map(|i| dict.child_value(i))
        .find(|entry| entry.child_value(0).str() == Some(key))
        .and_then(|entry| entry.child_value(1).as_variant())
}

/// Monitors from Mutter's DisplayConfig (session bus).
fn displays() -> Vec<Display> {
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return Vec::new();
    };
    let Ok(state) = bus.call_sync(
        Some("org.gnome.Mutter.DisplayConfig"),
        "/org/gnome/Mutter/DisplayConfig",
        "org.gnome.Mutter.DisplayConfig",
        "GetCurrentState",
        None,
        None,
        gio::DBusCallFlags::NONE,
        2000,
        gio::Cancellable::NONE,
    ) else {
        return Vec::new();
    };
    // (serial, monitors a((ssss)a(siiddada{sv})a{sv}), logical a(iiduba(ssss)a{sv}), props)
    let monitors = state.child_value(1);
    let logical = state.child_value(2);
    let scale_of = |connector: &str| -> f64 {
        (0..logical.n_children())
            .map(|i| logical.child_value(i))
            .find(|l| {
                let specs = l.child_value(5);
                (0..specs.n_children())
                    .any(|j| specs.child_value(j).child_value(0).str() == Some(connector))
            })
            .and_then(|l| l.child_value(2).get::<f64>())
            .unwrap_or(1.0)
    };
    (0..monitors.n_children())
        .filter_map(|i| {
            let m = monitors.child_value(i);
            let spec = m.child_value(0);
            let connector = spec.child_value(0).str()?.to_string();
            let modes = m.child_value(1);
            let props = m.child_value(2);
            let prop = |key: &str| lookup(&props, key);
            let current = (0..modes.n_children())
                .map(|j| modes.child_value(j))
                .find(|mode| {
                    lookup(&mode.child_value(6), "is-current")
                        .and_then(|v| v.get::<bool>())
                        .unwrap_or(false)
                })?;
            Some(Display {
                name: prop("display-name")
                    .and_then(|v| v.str().map(str::to_string))
                    .unwrap_or_else(|| connector.clone()),
                width: current.child_value(1).get::<i32>()? as u32,
                height: current.child_value(2).get::<i32>()? as u32,
                refresh: current.child_value(3).get::<f64>()?,
                scale: scale_of(&connector),
                builtin: prop("is-builtin")
                    .and_then(|v| v.get::<bool>())
                    .unwrap_or(false),
            })
        })
        .collect()
}

/// This machine.
pub struct RealProbe {
    pub runner: std::sync::Arc<dyn Runner>,
}

impl SysProbe for RealProbe {
    fn probe(&self) -> SysInfo {
        let uname = rustix::system::uname();
        let (model, vendor) = model(Path::new("/sys/class/dmi/id"));
        let chassis = read_num("/sys/class/dmi/id/chassis_type").unwrap_or(0.0) as u32;
        let (cpu, cores, threads) =
            parse_cpuinfo(&std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default());
        let memory = parse_meminfo(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default())
            .map(|(total, available, swap_total, swap_free)| Memory {
                total,
                available,
                swap_total,
                swap_free,
                pressure: read("/proc/pressure/memory").and_then(|t| parse_pressure(&t)),
            });
        let disk = disk();
        let battery = battery(Path::new("/sys/class/power_supply"));
        SysInfo {
            host_name: uname.nodename().to_string_lossy().into_owned(),
            user_name: glib::user_name().to_string_lossy().into_owned(),
            model,
            vendor,
            // DMI chassis types for portables: laptop, notebook, sub-notebook,
            // convertible, detachable, and so on.
            laptop: matches!(chassis, 8 | 9 | 10 | 14 | 30 | 31 | 32) || battery.is_some(),
            cpu,
            cores,
            threads,
            architecture: std::env::consts::ARCH.to_string(),
            os: read("/etc/os-release").and_then(|t| key_value(&t, "PRETTY_NAME")),
            kernel: uname.release().to_string_lossy().into_owned(),
            uptime: read("/proc/uptime")
                .and_then(|t| t.split_whitespace().next()?.parse::<f64>().ok())
                .map(|s| s as u64),
            shell: std::env::var("SHELL")
                .ok()
                .and_then(|s| Some(Path::new(&s).file_name()?.to_string_lossy().into_owned())),
            time_zone: read("/etc/timezone").or_else(|| {
                let link = std::fs::read_link("/etc/localtime").ok()?;
                let text = link.to_string_lossy();
                Some(text.split("zoneinfo/").nth(1)?.to_string())
            }),
            language: std::env::var("LANG")
                .ok()
                .map(|l| l.split('.').next().unwrap_or(&l).to_string())
                .filter(|l| !l.is_empty()),
            in_use_since: in_use_since(),
            battery,
            memory,
            temperature: temperature(),
            security: security(disk.as_ref()),
            disk,
            displays: displays(),
            packages: package_counts(&*self.runner),
        }
    }
}

// --- the fixture -------------------------------------------------------------------

/// Reads `SysInfo` from JSON (tests/data/sysinfo): the same field names.
pub struct FixtureProbe {
    pub path: PathBuf,
}

fn s(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

impl SysProbe for FixtureProbe {
    fn probe(&self) -> SysInfo {
        let text = std::fs::read_to_string(&self.path).unwrap_or_default();
        let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let u64_ = |v: &Value| v.as_u64();
        let u32_ = |v: &Value| v.as_u64().map(|n| n as u32);
        let f = |v: &Value| v.as_f64();
        let b = |v: &Value| v.as_bool();
        let battery = j.get("battery").filter(|v| !v.is_null()).map(|v| Battery {
            charge: f(&v["charge"]).unwrap_or(0.0),
            state: s(&v["state"]).unwrap_or_default(),
            full_wh: f(&v["full_wh"]),
            design_wh: f(&v["design_wh"]),
            cycles: u32_(&v["cycles"]),
            plugged_in: b(&v["plugged_in"]).unwrap_or(false),
            power_w: f(&v["power_w"]),
        });
        let memory = j.get("memory").filter(|v| !v.is_null()).map(|v| Memory {
            total: u64_(&v["total"]).unwrap_or(0),
            available: u64_(&v["available"]).unwrap_or(0),
            swap_total: u64_(&v["swap_total"]).unwrap_or(0),
            swap_free: u64_(&v["swap_free"]).unwrap_or(0),
            pressure: f(&v["pressure"]),
        });
        let disk = j.get("disk").filter(|v| !v.is_null()).map(|v| Disk {
            size: u64_(&v["size"]).unwrap_or(0),
            free: u64_(&v["free"]).unwrap_or(0),
            filesystem: s(&v["filesystem"]).unwrap_or_default(),
            drive: s(&v["drive"]),
            encrypted: b(&v["encrypted"]),
        });
        let sec = &j["security"];
        SysInfo {
            host_name: s(&j["host_name"]).unwrap_or_default(),
            user_name: s(&j["user_name"]).unwrap_or_default(),
            model: s(&j["model"]),
            vendor: s(&j["vendor"]),
            laptop: b(&j["laptop"]).unwrap_or(false),
            cpu: s(&j["cpu"]),
            cores: u32_(&j["cores"]),
            threads: u32_(&j["threads"]),
            architecture: s(&j["architecture"]).unwrap_or_default(),
            os: s(&j["os"]),
            kernel: s(&j["kernel"]).unwrap_or_default(),
            uptime: u64_(&j["uptime"]),
            shell: s(&j["shell"]),
            time_zone: s(&j["time_zone"]),
            language: s(&j["language"]),
            in_use_since: j["in_use_since"].as_i64(),
            battery,
            memory,
            temperature: f(&j["temperature"]),
            disk,
            displays: j["displays"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .map(|d| Display {
                            name: s(&d["name"]).unwrap_or_default(),
                            width: u32_(&d["width"]).unwrap_or(0),
                            height: u32_(&d["height"]).unwrap_or(0),
                            refresh: f(&d["refresh"]).unwrap_or(0.0),
                            scale: f(&d["scale"]).unwrap_or(1.0),
                            builtin: b(&d["builtin"]).unwrap_or(false),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            security: Security {
                disk_encryption: b(&sec["disk_encryption"]),
                firewall: b(&sec["firewall"]),
                secure_boot: b(&sec["secure_boot"]),
                apparmor: b(&sec["apparmor"]),
                auto_updates: b(&sec["auto_updates"]),
            },
            packages: PackageCounts {
                apt: u64_(&j["packages"]["apt"]),
                snap: u64_(&j["packages"]["snap"]),
                flatpak: u64_(&j["packages"]["flatpak"]),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpuinfo_counts_cores_and_threads() {
        let text = "processor\t: 0\nmodel name\t: Example  CPU 9000\nphysical id\t: 0\ncore id\t: 0\n\n\
                    processor\t: 1\nmodel name\t: Example  CPU 9000\nphysical id\t: 0\ncore id\t: 0\n\n\
                    processor\t: 2\nmodel name\t: Example  CPU 9000\nphysical id\t: 0\ncore id\t: 1\n";
        assert_eq!(
            parse_cpuinfo(text),
            (Some("Example CPU 9000".into()), Some(2), Some(3))
        );
    }

    #[test]
    fn meminfo_and_pressure() {
        let text = "MemTotal:       32768000 kB\nMemFree: 1 kB\nMemAvailable:   16384000 kB\nSwapTotal: 8 kB\nSwapFree: 4 kB\n";
        assert_eq!(
            parse_meminfo(text),
            Some((32_768_000 * 1024, 16_384_000 * 1024, 8 * 1024, 4 * 1024))
        );
        assert_eq!(
            parse_pressure("some avg10=1.25 avg60=0.50 avg300=0.10 total=1\nfull avg10=0.00\n"),
            Some(1.25)
        );
    }

    #[test]
    fn this_machine_can_be_probed_without_root() {
        let info = RealProbe {
            runner: std::sync::Arc::new(crate::tools::System),
        }
        .probe();
        assert!(!info.kernel.is_empty());
        assert!(info.memory.is_some());
        assert!(info.disk.is_some());
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    #[test]
    fn the_fixture_probe_reads_every_field() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/sysinfo/fixture.json");
        let info = FixtureProbe { path }.probe();
        assert_eq!(info.host_name, "studio-desk");
        assert_eq!(info.model.as_deref(), Some("ThinkPad X1 Carbon Gen 13"));
        assert_eq!(info.cores, Some(8));
        let battery = info.battery.as_ref().unwrap();
        assert_eq!(battery.cycles, Some(110));
        assert_eq!(battery.capacity().map(|c| c.round()), Some(97.0));
        assert_eq!(info.disk.as_ref().unwrap().encrypted, Some(true));
        assert_eq!(info.displays.len(), 2);
        assert_eq!(info.security.firewall, Some(false));
        assert_eq!(info.packages.apt, Some(1850));
        assert_eq!(info.in_use_since, Some(1_740_830_400));
    }
}
