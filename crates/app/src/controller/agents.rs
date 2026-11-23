//! AI Agents (P7-T06; reference: ai-agents.png): token usage over a period,
//! the local servers on this machine with Stop, and the projects set up for
//! coding agents with their files and MCP servers. Read on this machine;
//! nothing is sent anywhere.

use std::path::Path;

use gettextrs::gettext;
use serde_json::{Value, json};

use steward_core::agents::{self, AGENT_FILES, Agents};
use steward_core::servers::{self, Server};

use super::{Controller, Event, dirty, later};
use crate::{format, paths};

#[derive(Default, Debug)]
pub struct AgentsState {
    found: Option<Agents>,
    servers: Vec<Server>,
    looking: bool,
    /// "today", "week" or "month".
    period: String,
    ticket: u64,
    /// The server waiting for "Stop?" to be confirmed (pid).
    asking: Option<u32>,
    servers_timer: Option<glib::SourceId>,
}

fn tool_name(id: &str) -> &'static str {
    AGENT_FILES
        .iter()
        .find(|(t, _, _)| *t == id)
        .map(|(_, n, _)| *n)
        .unwrap_or("")
}

/// "1.2M", "38.6K", "950".
fn compact(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{:.1}K", n as f64 / 1e3),
        n => n.to_string(),
    }
}

fn duration(secs: i64) -> String {
    let secs = secs.max(0);
    match secs {
        s if s >= 86_400 => format!("{} d", s / 86_400),
        s if s >= 3600 => format!("{} h", s / 3600),
        s => format!("{} min", (s / 60).max(1)),
    }
}

impl Controller {
    pub fn agents_opened(&mut self) {
        if self.agents.found.is_none() && !self.agents.looking {
            self.agents_refresh();
        }
        // Servers come and go: look every few seconds while the page is open.
        if self.agents.servers_timer.is_none() {
            let id = glib::timeout_add_seconds_local(3, || {
                later(|c| {
                    if c.tools.page == "agents" {
                        c.agents_servers();
                    }
                });
                glib::ControlFlow::Continue
            });
            self.agents.servers_timer = Some(id);
        }
        self.agents_servers();
    }

    pub fn agents_refresh(&mut self) {
        self.agents.looking = true;
        self.agents.ticket += 1;
        let ticket = self.agents.ticket;
        self.mark(dirty::TOOLS);
        let home = glib::home_dir();
        std::thread::spawn(move || {
            let found = agents::scan(&home);
            glib::MainContext::default().invoke(move || {
                later(move |c| {
                    if c.agents.ticket == ticket {
                        c.agents.looking = false;
                        c.agents.found = Some(found);
                        c.mark(dirty::TOOLS);
                    }
                })
            });
        });
        self.agents_servers();
    }

    fn agents_servers(&mut self) {
        let now = servers::listening(Path::new("/proc"));
        if now != self.agents.servers {
            self.agents.servers = now;
            self.mark(dirty::TOOLS);
        }
    }

    pub fn agents_period(&mut self, period: &str) {
        self.agents.period = period.to_string();
        self.mark(dirty::TOOLS);
    }

    /// Stop: asks first.
    pub fn agents_stop_ask(&mut self, pid: &str) {
        let Ok(pid) = pid.parse::<u32>() else { return };
        let Some(server) = self.agents.servers.iter().find(|s| s.pid == pid) else {
            return;
        };
        self.agents.asking = Some(pid);
        self.dialog_data = json!({
            "pid": pid, "port": server.port, "program": server.program,
            "project": server.cwd.as_deref().map(paths::display_path).unwrap_or_default(),
        });
        self.show_dialog("stop-server");
    }

    pub fn agents_stop(&mut self, confirmed: bool) {
        let asked = self.agents.asking.take();
        if self.dialog == "stop-server" {
            self.close_dialog();
        }
        let (true, Some(pid)) = (confirmed, asked) else {
            return;
        };
        match servers::stop(pid) {
            Ok(()) => self.emit(Event::Toast(gettext("Asked the server to stop"), false)),
            Err(error) => self.emit(Event::Toast(
                format!("{}: {error}", gettext("Could not stop it")),
                false,
            )),
        }
        self.agents_servers();
    }

    /// Opens http://localhost:<port>/ in the browser.
    pub fn agents_open(&mut self, port: &str) {
        let url = format!("http://localhost:{port}/");
        if let Err(error) =
            gio::AppInfo::launch_default_for_uri(&url, None::<&gio::AppLaunchContext>)
        {
            self.emit(Event::Toast(
                format!("{}: {error}", gettext("Could not open it")),
                false,
            ));
        }
    }

    fn agents_usage(&self, found: &Agents) -> Value {
        let period = if self.agents.period.is_empty() {
            "week"
        } else {
            self.agents.period.as_str()
        };
        let days = match period {
            "today" => 1,
            "month" => 30,
            _ => 7,
        };
        let now = steward_core::plan::now();
        let bars: Vec<(String, u64)> = (0..days)
            .rev()
            .map(|back| {
                let day = glib::DateTime::from_unix_utc(now - back * 86_400)
                    .ok()
                    .and_then(|d| d.format("%Y-%m-%d").ok())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let total = found.days.get(&day).map(|t| t.total()).unwrap_or(0);
                (day, total)
            })
            .collect();
        let total: u64 = bars.iter().map(|b| b.1).sum();
        let max = bars.iter().map(|b| b.1).max().unwrap_or(0).max(1);
        let phrase = match period {
            "today" => gettext("tokens today"),
            "month" => gettext("tokens in the last 30 days"),
            _ => gettext("tokens this week"),
        };
        json!({
            "period": period,
            "total": total,
            "totalText": compact(total),
            "caption": phrase,
            "bars": bars.iter().map(|(day, n)| json!({
                "day": day, "value": n, "share": *n as f64 / max as f64, "label": compact(*n),
            })).collect::<Vec<_>>(),
            "legend": [{ "tool": "Claude Code", "text": compact(total) }],
            "note": gettext("Usage is read from Claude Code's logs; other tools keep no local usage Steward can read yet."),
        })
    }

    pub fn agents_json(&self) -> Value {
        // Servers are live processes: their uptime is against the real clock.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let servers: Vec<Value> = self
            .agents
            .servers
            .iter()
            .map(|s| {
                let project = s
                    .cwd
                    .as_ref()
                    .and_then(|c| c.file_name())
                    .map(|n| n.to_string_lossy().into_owned());
                json!({
                    "port": s.port, "pid": s.pid, "program": s.program, "command": s.command,
                    "project": project.unwrap_or_default(),
                    "uptime": s.started.map(|t| duration(now - t)).unwrap_or_default(),
                })
            })
            .collect();
        let Some(found) = &self.agents.found else {
            return json!({ "looking": true, "servers": servers, "projects": [] });
        };
        let projects: Vec<Value> = found
            .projects
            .iter()
            .map(|p| {
                json!({
                    "name": p.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    "path": p.path.display().to_string(),
                    "where": p.path.parent().map(paths::display_path).unwrap_or_default(),
                    "tools": p.tools.iter().map(|t| tool_name(t)).collect::<Vec<_>>(),
                    "files": p.files.iter().map(|(_, f)| f.clone()).collect::<Vec<_>>(),
                    "servers": p.servers.iter().map(|s| json!({ "name": s.name, "scope": s.scope, "command": s.command })).collect::<Vec<_>>(),
                    "tokens": p.tokens.total(),
                    "tokensText": if p.tokens.total() > 0 { format!("{} {}", compact(p.tokens.total()), gettext("tokens")) } else { String::new() },
                    "sessions": p.sessions,
                    "last": p.last.map(format::date).unwrap_or_default(),
                })
            })
            .collect();
        json!({
            "looking": self.agents.looking,
            "usage": self.agents_usage(found),
            "servers": servers,
            "projects": projects,
            "userServers": found.user_servers.iter().map(|s| json!({ "name": s.name, "command": s.command })).collect::<Vec<_>>(),
        })
    }

    #[cfg(feature = "e2e")]
    pub fn agents_test_json(&self) -> Value {
        let page = self.agents_json();
        json!({
            "loaded": self.agents.found.is_some() && !self.agents.looking,
            "usage_total": page["usage"]["total"],
            "period": page["usage"]["period"],
            "all_days": self.agents.found.as_ref().map(|f| f.days.iter().map(|(d, t)| (d.clone(), json!(t.total())))
                .collect::<serde_json::Map<_, _>>()),
            "projects": page["projects"],
            "servers": page["servers"],
            "user_servers": page["userServers"].as_array().map(Vec::len),
        })
    }
}
