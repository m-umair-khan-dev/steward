//! L1 for `core::agents` (P7-T04) on sample data in tests/data/agents.

use std::path::{Path, PathBuf};

use cairn_core::agents::{self, Tokens};

fn data(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/agents").join(name)).unwrap()
}

#[test]
fn a_session_counts_each_response_once() {
    let s = agents::parse_session(&data("session.jsonl"));
    assert_eq!(s.cwd, Some(PathBuf::from("/home/sam/Documents/Projects/web-app")));
    assert_eq!(s.days["2026-10-30"], Tokens { input: 1200, output: 350, cache_write: 5000, cache_read: 0 },
               "msg_01 counts once, with its last usage");
    assert_eq!(s.days["2026-10-31"].total(), 7000);
    assert_eq!(s.last, glib::DateTime::from_iso8601("2026-10-31T10:00:00Z", None).ok().map(|d| d.to_unix()));
}

#[test]
fn mcp_servers_by_scope() {
    let (user, local) = agents::parse_claude_json(&data("claude.json"));
    assert_eq!(user.len(), 1);
    assert_eq!(user[0].command, "http://localhost:9000/mcp");
    let web = &local[Path::new("/home/sam/Documents/Projects/web-app")];
    assert_eq!((web[0].name.as_str(), web[0].scope, web[0].command.as_str()), ("playwright", "local", "npx @playwright/mcp"));
    assert!(local[Path::new("/home/sam/Documents/Projects/api")].is_empty());
    let project = agents::parse_mcp_json(&data("mcp.json"));
    assert_eq!(project.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["github", "postgres"]);
    assert_eq!(project[0].command, "github-mcp --read-only");
    assert!(agents::parse_mcp_json("not json").is_empty());
}
