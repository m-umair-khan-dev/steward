//! L2 for `core::agents` (P7-T04): the fixture's planted Claude Code setup
//! against its manifest.

use std::path::Path;
use std::process::Command;

#[test]
fn the_fixture_agents_match_the_manifest() {
    let root = std::env::temp_dir().join(format!("cairn-agents-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docker/scripts/make-fixture-home");
    assert!(
        Command::new("python3")
            .arg("-I")
            .arg(script)
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(".cairn-fixture")).unwrap())
            .unwrap();
    let expected = &manifest["agents"];
    let agents = cairn_core::agents::scan(&root);

    let found: Vec<String> = agents
        .projects
        .iter()
        .map(|p| p.path.strip_prefix(&root).unwrap().display().to_string())
        .collect();
    let mut want: Vec<String> = expected["projects"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    want.sort();
    assert_eq!(found, want);
    for project in &agents.projects {
        let rel = project
            .path
            .strip_prefix(&root)
            .unwrap()
            .display()
            .to_string();
        let facts = &expected["projects"][&rel];
        let tools: Vec<&str> = facts["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap())
            .collect();
        assert_eq!(project.tools, tools, "{rel} tools");
        let files: Vec<&str> = project.files.iter().map(|(_, f)| f.as_str()).collect();
        let want_files: Vec<&str> = facts["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap())
            .collect();
        assert_eq!(files, want_files, "{rel} files");
        let mut servers: Vec<&str> = project.servers.iter().map(|s| s.name.as_str()).collect();
        servers.sort();
        let mut want_servers: Vec<&str> = facts["servers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t.as_str().unwrap())
            .collect();
        want_servers.sort();
        assert_eq!(servers, want_servers, "{rel} servers");
        assert_eq!(
            project.tokens.total(),
            facts["tokens"].as_u64().unwrap(),
            "{rel} tokens"
        );
        assert_eq!(
            project.sessions as u64,
            facts["sessions"].as_u64().unwrap(),
            "{rel} sessions"
        );
    }
    assert_eq!(agents.user_servers.len(), 1);
    let total: u64 = agents.days.values().map(|t| t.total()).sum();
    assert_eq!(total, expected["tokens_total"].as_u64().unwrap());
    for (day, tokens) in expected["days"].as_object().unwrap() {
        assert_eq!(agents.days[day].total(), tokens.as_u64().unwrap(), "{day}");
    }
    let _ = std::fs::remove_dir_all(&root);
}
