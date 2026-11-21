# Sample coding-agent data

Invented files in the formats verified in ADR 0013, for the parser tests in
`crates/core/tests/agents.rs`. They contain no real prompts or keys.

| File | Format of |
|---|---|
| `session.jsonl` | A Claude Code session log (`~/.claude/projects/<folder>/<id>.jsonl`): `cwd`, timestamps, `message.usage`; one response written twice while streaming (`msg_01`); a broken line |
| `claude.json` | `~/.claude.json`: user-scope `mcpServers` and per-project `mcpServers` |
| `mcp.json` | A project's `.mcp.json` |
