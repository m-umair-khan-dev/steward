//! AI Agents (P7-T04; DESIGN.md §4.2; ADR 0013): projects set up for coding
//! agents, their agent files, MCP servers, and token usage from local logs.
//! Only counts, names and paths are read; prompts and replies never are.
//!
//! Token usage and MCP servers come from Claude Code, the one tool verified
//! on Linux (ADR 0013). Agent files are recognised for every common tool.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Agent files by tool: (tool id, display name, file or folder names).
pub const AGENT_FILES: &[(&str, &str, &[&str])] = &[
    (
        "claude",
        "Claude Code",
        &["CLAUDE.md", ".claude", ".mcp.json"],
    ),
    ("codex", "Codex", &["AGENTS.md"]),
    ("gemini", "Gemini CLI", &["GEMINI.md"]),
    ("cursor", "Cursor", &[".cursor/rules", ".cursorrules"]),
    (
        "copilot",
        "GitHub Copilot",
        &[".github/copilot-instructions.md"],
    ),
    ("windsurf", "Windsurf", &[".windsurfrules"]),
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_write: u64,
    pub cache_read: u64,
}

impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_write + self.cache_read
    }

    fn add(&mut self, other: &Tokens) {
        self.input += other.input;
        self.output += other.output;
        self.cache_write += other.cache_write;
        self.cache_read += other.cache_read;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServer {
    pub name: String,
    /// "project" (.mcp.json), "local" (~/.claude.json for this project) or
    /// "user" (~/.claude.json for every project).
    pub scope: &'static str,
    /// The program it runs, or its URL.
    pub command: String,
}

/// One Claude Code session log, read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    pub cwd: Option<PathBuf>,
    /// Usage per day ("2026-10-31"), each response counted once.
    pub days: BTreeMap<String, Tokens>,
    /// The last time anything happened (seconds).
    pub last: Option<i64>,
}

fn tokens_of(usage: &Value) -> Tokens {
    let n = |k: &str| usage[k].as_u64().unwrap_or(0);
    Tokens {
        input: n("input_tokens"),
        output: n("output_tokens"),
        cache_write: n("cache_creation_input_tokens"),
        cache_read: n("cache_read_input_tokens"),
    }
}

fn seconds(timestamp: &str) -> Option<i64> {
    glib::DateTime::from_iso8601(timestamp, Some(&glib::TimeZone::utc()))
        .ok()
        .map(|d| d.to_unix())
}

/// Reads a session log (JSON lines). A response streamed in parts is
/// written more than once with the same `message.id`: it counts once, with
/// its last usage.
pub fn parse_session(text: &str) -> Session {
    let mut session = Session::default();
    let mut seen: HashMap<String, (String, Tokens)> = HashMap::new();
    let mut anonymous: Vec<(String, Tokens)> = Vec::new();
    for line in text.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if session.cwd.is_none()
            && let Some(cwd) = entry["cwd"].as_str()
        {
            session.cwd = Some(PathBuf::from(cwd));
        }
        let when = entry["timestamp"].as_str().unwrap_or("");
        if let Some(t) = seconds(when) {
            session.last = Some(session.last.map_or(t, |l| l.max(t)));
        }
        let message = &entry["message"];
        if entry["type"] != "assistant" || !message["usage"].is_object() {
            continue;
        }
        let day = when.get(..10).unwrap_or("").to_string();
        let tokens = tokens_of(&message["usage"]);
        match message["id"].as_str() {
            Some(id) => {
                seen.insert(id.to_string(), (day, tokens));
            }
            None => anonymous.push((day, tokens)),
        }
    }
    for (day, tokens) in seen.into_values().chain(anonymous) {
        session.days.entry(day).or_default().add(&tokens);
    }
    session
}

fn server_command(config: &Value) -> String {
    if let Some(url) = config["url"].as_str() {
        return url.to_string();
    }
    let command = config["command"].as_str().unwrap_or("");
    let args: Vec<&str> = config["args"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    std::iter::once(command)
        .chain(args)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn servers(map: &Value, scope: &'static str) -> Vec<McpServer> {
    let mut out: Vec<McpServer> = map
        .as_object()
        .map(|servers| {
            servers
                .iter()
                .map(|(name, config)| McpServer {
                    name: name.clone(),
                    scope,
                    command: server_command(config),
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// `~/.claude.json`: user-scope servers, and each project's local ones.
pub fn parse_claude_json(text: &str) -> (Vec<McpServer>, BTreeMap<PathBuf, Vec<McpServer>>) {
    let Ok(json) = serde_json::from_str::<Value>(text) else {
        return (Vec::new(), BTreeMap::new());
    };
    let projects = json["projects"]
        .as_object()
        .map(|p| {
            p.iter()
                .map(|(path, project)| {
                    (
                        PathBuf::from(path),
                        servers(&project["mcpServers"], "local"),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    (servers(&json["mcpServers"], "user"), projects)
}

/// A project's `.mcp.json`.
pub fn parse_mcp_json(text: &str) -> Vec<McpServer> {
    serde_json::from_str::<Value>(text)
        .map(|json| servers(&json["mcpServers"], "project"))
        .unwrap_or_default()
}

/// The agent files in a project folder: (tool id, relative path).
pub fn agent_files(project: &Path) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for (tool, _, names) in AGENT_FILES {
        for name in *names {
            if project.join(name).symlink_metadata().is_ok() {
                out.push((*tool, name.to_string()));
            }
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub path: PathBuf,
    /// Tools with agent files or logs here, by id.
    pub tools: Vec<&'static str>,
    pub files: Vec<(&'static str, String)>,
    pub servers: Vec<McpServer>,
    pub tokens: Tokens,
    pub sessions: usize,
    pub last: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Agents {
    pub projects: Vec<Project>,
    pub user_servers: Vec<McpServer>,
    /// Claude Code usage per day, over every project.
    pub days: BTreeMap<String, Tokens>,
}

/// Folders under `home` (not hidden, at most `depth` deep) that hold agent
/// files.
fn find_projects(home: &Path, depth: usize) -> Vec<PathBuf> {
    let skip = ["node_modules", "target", "build", "dist", "vendor"];
    let mut out = Vec::new();
    let mut stack = vec![(home.to_path_buf(), 0)];
    while let Some((dir, level)) = stack.pop() {
        if dir != home && !agent_files(&dir).is_empty() {
            out.push(dir.clone());
        }
        if level >= depth {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || skip.contains(&name.as_str()) {
                continue;
            }
            if entry.path().symlink_metadata().is_ok_and(|m| m.is_dir()) {
                stack.push((entry.path(), level + 1));
            }
        }
    }
    out
}

/// A path as a log or config records it: absolute, or "~/…" (the fixture
/// writes that so it can be copied anywhere; real logs are absolute).
fn resolve(path: &Path, home: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => home.join(rest),
        Err(_) => path.to_path_buf(),
    }
}

/// Everything the view shows, read from `home`.
pub fn scan(home: &Path) -> Agents {
    let mut agents = Agents::default();
    let mut per_project: BTreeMap<PathBuf, (Tokens, usize, Option<i64>)> = BTreeMap::new();
    if let Ok(folders) = std::fs::read_dir(home.join(".claude/projects")) {
        for folder in folders.flatten() {
            let Ok(logs) = std::fs::read_dir(folder.path()) else {
                continue;
            };
            for log in logs.flatten() {
                if log.path().extension().is_none_or(|e| e != "jsonl") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(log.path()) else {
                    continue;
                };
                let session = parse_session(&text);
                let Some(cwd) = session.cwd.as_deref().map(|c| resolve(c, home)) else {
                    continue;
                };
                let entry = per_project.entry(cwd).or_default();
                for (day, tokens) in &session.days {
                    entry.0.add(tokens);
                    agents.days.entry(day.clone()).or_default().add(tokens);
                }
                entry.1 += 1;
                entry.2 = match (entry.2, session.last) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (a, b) => a.or(b),
                };
            }
        }
    }
    let (user_servers, local) = std::fs::read_to_string(home.join(".claude.json"))
        .map(|t| parse_claude_json(&t))
        .unwrap_or_default();
    agents.user_servers = user_servers;
    let local: BTreeMap<PathBuf, Vec<McpServer>> = local
        .into_iter()
        .map(|(p, s)| (resolve(&p, home), s))
        .collect();

    let mut paths: HashSet<PathBuf> = per_project.keys().cloned().collect();
    paths.extend(
        local
            .iter()
            .filter(|(_, s)| !s.is_empty())
            .map(|(p, _)| p.clone()),
    );
    paths.extend(find_projects(home, 5));
    let mut paths: Vec<PathBuf> = paths
        .into_iter()
        .filter(|p| p.is_dir() && p != home)
        .collect();
    paths.sort();

    for path in paths {
        let files = agent_files(&path);
        let mut servers = std::fs::read_to_string(path.join(".mcp.json"))
            .map(|t| parse_mcp_json(&t))
            .unwrap_or_default();
        servers.extend(local.get(&path).cloned().unwrap_or_default());
        let (tokens, sessions, last) = per_project.get(&path).copied().unwrap_or_default();
        let mut tools: Vec<&'static str> = files.iter().map(|(t, _)| *t).collect();
        if sessions > 0 || !servers.is_empty() {
            tools.push("claude");
        }
        tools.sort_by_key(|t| AGENT_FILES.iter().position(|(id, _, _)| id == t));
        tools.dedup();
        agents.projects.push(Project {
            path,
            tools,
            files,
            servers,
            tokens,
            sessions,
            last,
        });
    }
    agents
}
