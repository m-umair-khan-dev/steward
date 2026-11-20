//! About This Computer (P5-T11; reference: about-this-mac.gif), opened from
//! the sidebar's computer row: the machine at a glance, then battery,
//! startup disk, memory, displays, security and software. Hide Names masks
//! the computer's and the user's names; Copy as Text copies what is shown.

use std::sync::Arc;

use gettextrs::gettext;
use serde_json::{Value, json};

use cairn_core::sysinfo::{RealProbe, SysInfo, SysProbe};

use super::{Controller, Event, dirty, later};
use crate::format;

#[derive(Default, Debug)]
pub struct AboutState {
    info: Option<SysInfo>,
    looking: bool,
    hide_names: bool,
    /// The last text copied (tests read it).
    copied: Option<String>,
}

const MASK: &str = "••••••";

/// "6 days, 15 hours"; under a day, "3 hours, 12 minutes".
fn uptime_text(secs: u64) -> String {
    let (days, hours, minutes) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
    let unit = |n: u64, one: &str, many: &str| {
        format!("{n} {}", if n == 1 { gettext(one) } else { gettext(many) })
    };
    if days > 0 {
        format!(
            "{}, {}",
            unit(days, "day", "days"),
            unit(hours, "hour", "hours")
        )
    } else {
        format!(
            "{}, {}",
            unit(hours, "hour", "hours"),
            unit(minutes, "minute", "minutes")
        )
    }
}

/// "1 year, 8 months" since `since`.
fn age_text(since: i64, now: i64) -> String {
    let months = ((now - since).max(0) as f64 / (30.436_875 * 86_400.0)).floor() as i64;
    let (years, months) = (months / 12, months % 12);
    let unit = |n: i64, one: &str, many: &str| {
        format!("{n} {}", if n == 1 { gettext(one) } else { gettext(many) })
    };
    match (years, months) {
        (0, m) => unit(m.max(1), "month", "months"),
        (y, 0) => unit(y, "year", "years"),
        (y, m) => format!(
            "{}, {}",
            unit(y, "year", "years"),
            unit(m, "month", "months")
        ),
    }
}

fn on_off(value: Option<bool>) -> String {
    match value {
        Some(true) => gettext("On"),
        Some(false) => gettext("Off"),
        None => gettext("Unknown"),
    }
}

fn percent(value: f64) -> String {
    format!("{}%", value.round() as i64)
}

fn row(label: String, value: String) -> Value {
    json!({ "label": label, "value": value })
}

fn stat(label: String, value: String) -> Value {
    json!({ "label": label, "value": value })
}

impl Controller {
    pub fn about_opened(&mut self) {
        if self.about.info.is_none() && !self.about.looking {
            self.about_refresh();
        }
    }

    fn about_refresh(&mut self) {
        self.about.looking = true;
        self.mark(dirty::TOOLS);
        let probe: Box<dyn SysProbe> = probe();
        std::thread::spawn(move || {
            let info = probe.probe();
            glib::MainContext::default().invoke(move || {
                later(move |c| {
                    c.about.looking = false;
                    c.about.info = Some(info);
                    c.mark(dirty::TOOLS);
                })
            });
        });
    }

    pub fn about_hide_names(&mut self, hide: bool) {
        self.about.hide_names = hide;
        self.mark(dirty::TOOLS);
    }

    /// Copy as Text: what the page shows, as plain lines.
    pub fn about_copy(&mut self) {
        let page = self.about_json();
        if page["looking"] == true {
            return;
        }
        let mut text = format!(
            "{}\n{}\n{}\n",
            page["title"].as_str().unwrap_or(""),
            page["subtitle"].as_str().unwrap_or(""),
            page["chips"]
                .as_array()
                .map(|c| {
                    c.iter()
                        .map(|c| {
                            format!(
                                "{} {}",
                                c["label"].as_str().unwrap_or(""),
                                c["value"].as_str().unwrap_or("")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" · ")
                })
                .unwrap_or_default()
        );
        for section in page["sections"].as_array().into_iter().flatten() {
            text.push_str(&format!("\n{}\n", section["title"].as_str().unwrap_or("")));
            for line in section["stats"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(section["rows"].as_array().into_iter().flatten())
            {
                text.push_str(&format!(
                    "  {}: {}\n",
                    line["label"].as_str().unwrap_or(""),
                    line["value"].as_str().unwrap_or("")
                ));
            }
        }
        crate::bridge::qobject::clipboard_set(
            &vec!["text/plain;charset=utf-8".into(), "text/plain".into()],
            &vec![text.clone(), text.clone()],
        );
        self.about.copied = Some(text);
        self.emit(Event::Toast(gettext("Copied as text"), false));
    }

    pub fn about_json(&self) -> Value {
        let Some(info) = &self.about.info else {
            return json!({ "looking": true, "sections": [] });
        };
        let names = |name: &str| -> String {
            if self.about.hide_names {
                MASK.into()
            } else {
                name.to_string()
            }
        };
        let unknown = || gettext("Unknown");
        let now = cairn_core::plan::now();
        let title = info
            .model
            .clone()
            .unwrap_or_else(|| gettext("This computer"));
        let mut subtitle: Vec<String> = Vec::new();
        subtitle.extend(info.os.clone());
        subtitle.extend(info.cpu.clone());
        if let Some(m) = &info.memory {
            subtitle.push(format!("{} {}", format::size(m.total), gettext("memory")));
        }
        if let Some(d) = &info.disk {
            subtitle.push(format!("{} {}", format::size(d.size), gettext("storage")));
        }
        let condition = |b: &cairn_core::sysinfo::Battery| match b.capacity() {
            Some(c) if c < 80.0 => gettext("Service recommended"),
            Some(_) => gettext("Normal"),
            None => unknown(),
        };
        let mut chips = Vec::new();
        if let Some(up) = info.uptime {
            chips.push(
                json!({ "icon": "progress", "label": gettext("Up"), "value": uptime_text(up) }),
            );
        }
        if let Some(since) = info.in_use_since {
            chips.push(json!({ "icon": "documents", "label": gettext("In use"), "value": age_text(since, now) }));
        }
        if let Some(b) = &info.battery {
            chips.push(
                json!({ "icon": "progress", "label": gettext("Battery"), "value": condition(b) }),
            );
        }

        let mut sections = Vec::new();
        if let Some(b) = &info.battery {
            let state = match b.state.as_str() {
                "charging" => gettext("charging"),
                "discharging" => gettext("on battery"),
                "full" => gettext("fully charged"),
                _ => gettext("not charging"),
            };
            sections.push(json!({
                "id": "battery", "title": gettext("Battery health"), "icon": "progress",
                "stats": [
                    stat(gettext("Condition"), condition(b)),
                    stat(gettext("Maximum capacity"), b.capacity().map(percent).unwrap_or_else(unknown)),
                    stat(gettext("Cycles"), b.cycles.map(|c| c.to_string()).unwrap_or_else(unknown)),
                ],
                "bar": b.charge / 100.0,
                "caption": format!("{} {} · {state}", percent(b.charge), gettext("charged")),
                "rows": [
                    row(gettext("Full charge"), match (b.full_wh, b.design_wh) {
                        (Some(f), Some(d)) => format!("{f:.1} Wh {} {d:.1} Wh {}", gettext("of"), gettext("designed")),
                        (Some(f), None) => format!("{f:.1} Wh"),
                        _ => unknown(),
                    }),
                    row(gettext("Power"), format!("{}{}",
                        b.power_w.map(|w| format!("{w:.1} W · ")).unwrap_or_default(),
                        if b.plugged_in { gettext("Power adapter") } else { gettext("Battery") })),
                ],
            }));
        }
        if let Some(d) = &info.disk {
            let used = d.size.saturating_sub(d.free);
            sections.push(json!({
                "id": "disk", "title": gettext("Startup disk"), "icon": "drive",
                "stats": [
                    stat(gettext("Free"), format::size(d.free)),
                    stat(gettext("Used"), percent(used as f64 / d.size.max(1) as f64 * 100.0)),
                ],
                "bar": used as f64 / d.size.max(1) as f64,
                "caption": format!("{} {} {}", format::size(used), gettext("used of"), format::size(d.size)),
                "link": gettext("Free Up Space…"),
                "rows": [
                    row(gettext("File system"), d.filesystem.clone()),
                    row(gettext("Encrypted"), match d.encrypted {
                        Some(true) => gettext("Yes (LUKS)"),
                        Some(false) => gettext("No"),
                        None => unknown(),
                    }),
                    row(gettext("Drive"), d.drive.clone().unwrap_or_else(unknown)),
                ],
            }));
        }
        if let Some(m) = &info.memory {
            let used = m.total.saturating_sub(m.available);
            let pressure = match m.pressure {
                Some(p) if p >= 10.0 => gettext("High"),
                Some(p) if p >= 1.0 => gettext("Some"),
                Some(_) => gettext("Low"),
                None => unknown(),
            };
            sections.push(json!({
                "id": "memory", "title": gettext("Memory"), "icon": "box",
                "stats": [
                    stat(gettext("In use"), format::size(used)),
                    stat(gettext("Pressure"), pressure),
                ],
                "bar": used as f64 / m.total.max(1) as f64,
                "caption": format!("{} {} {}", format::size(used), gettext("of"), format::size(m.total)),
                "rows": [
                    row(gettext("Swap"), format!("{} {} {}", format::size(m.swap_total.saturating_sub(m.swap_free)),
                                                  gettext("used of"), format::size(m.swap_total))),
                    row(gettext("Thermal state"), match info.temperature {
                        Some(t) if t >= 85.0 => format!("{} ({t:.0} °C)", gettext("Hot")),
                        Some(t) if t >= 70.0 => format!("{} ({t:.0} °C)", gettext("Warm")),
                        Some(t) => format!("{} ({t:.0} °C)", gettext("Normal")),
                        None => unknown(),
                    }),
                ],
            }));
        }
        sections.push(json!({
            "id": "displays", "title": gettext("Displays"), "icon": "preview",
            "rows": if info.displays.is_empty() { vec![row(gettext("Displays"), unknown())] } else {
                info.displays.iter().map(|d| row(d.name.clone(), format!("{} × {} · {} Hz · {}%",
                    d.width, d.height, d.refresh.round(), (d.scale * 100.0).round()))).collect()
            },
        }));
        sections.push(json!({
            "id": "security", "title": gettext("Security"), "icon": "shield",
            "rows": [
                row(gettext("Disk encryption"), on_off(info.security.disk_encryption)),
                row(gettext("Firewall"), on_off(info.security.firewall)),
                row(gettext("Secure Boot"), on_off(info.security.secure_boot)),
                row(gettext("AppArmor"), on_off(info.security.apparmor)),
                row(gettext("Automatic security updates"), on_off(info.security.auto_updates)),
            ],
        }));
        sections.push(json!({
            "id": "computer", "title": gettext("Computer"), "icon": "computer",
            "rows": [
                row(gettext("Computer name"), names(&info.host_name)),
                row(gettext("User"), names(&info.user_name)),
                row(gettext("Processor"), info.cpu.clone().unwrap_or_else(unknown)),
                row(gettext("Cores"), match (info.cores, info.threads) {
                    (Some(c), Some(t)) if c != t => format!("{c} ({t} {})", gettext("threads")),
                    (Some(c), _) => c.to_string(),
                    _ => unknown(),
                }),
                row(gettext("Architecture"), info.architecture.clone()),
            ],
        }));
        let packages: Vec<String> = [
            (info.packages.apt, "apt"),
            (info.packages.snap, "Snap"),
            (info.packages.flatpak, "Flatpak"),
        ]
        .iter()
        .filter_map(|(n, tool)| n.map(|n| format!("{n} {tool}")))
        .collect();
        sections.push(json!({
            "id": "software", "title": gettext("Software"), "icon": "package",
            "rows": [
                row(gettext("System"), info.os.clone().unwrap_or_else(unknown)),
                row(gettext("Kernel"), info.kernel.clone()),
                row(gettext("Shell"), info.shell.clone().unwrap_or_else(unknown)),
                row(gettext("Region"), [info.time_zone.clone(), info.language.clone()].into_iter().flatten()
                    .collect::<Vec<_>>().join(" · ")),
                row(gettext("Packages"), if packages.is_empty() { unknown() } else { packages.join(" · ") }),
                row(gettext("In use since"), info.in_use_since.map(|s| format!("{} ({})", format::date(s),
                    gettext("estimate"))).unwrap_or_else(unknown)),
            ],
        }));
        json!({
            "looking": false,
            "title": title,
            "laptop": info.laptop,
            "subtitle": subtitle.join(" · "),
            "chips": chips,
            "sections": sections,
            "hideNames": self.about.hide_names,
        })
    }

    #[cfg(feature = "e2e")]
    pub fn about_test_json(&self) -> Value {
        let page = self.about_json();
        json!({
            "loaded": self.about.info.is_some(),
            "title": page["title"], "subtitle": page["subtitle"], "chips": page["chips"],
            "hide_names": self.about.hide_names,
            "rows": page["sections"].as_array().map(|s| s.iter().flat_map(|section| {
                section["stats"].as_array().into_iter().flatten()
                    .chain(section["rows"].as_array().into_iter().flatten())
                    .map(|r| (format!("{}/{}", section["id"].as_str().unwrap_or(""), r["label"].as_str().unwrap_or("")),
                              r["value"].clone()))
                    .collect::<Vec<_>>()
            }).collect::<serde_json::Map<_, _>>()),
            "copied": self.about.copied,
        })
    }
}

/// This machine, or (in test builds) the fixture named by
/// `CAIRN_SYSINFO_FIXTURE`.
fn probe() -> Box<dyn SysProbe> {
    #[cfg(feature = "e2e")]
    if let Some(path) = std::env::var_os("CAIRN_SYSINFO_FIXTURE") {
        return Box::new(cairn_core::sysinfo::FixtureProbe { path: path.into() });
    }
    Box::new(RealProbe {
        runner: Arc::new(cairn_core::tools::System),
    })
}
