# ADR 0013: Where coding agents keep their files on Linux

- **Status:** accepted
- **Date:** 2026-11-21
- **Task:** P7-S1
- **Design reference:** DESIGN.md §4.2 (AI Agents, **[verify paths per tool]**), Phase 7

## Context

The AI Agents view shows:
- projects set up for coding agents, with their agent files;
- MCP servers;
- token usage read from local logs.

DESIGN.md assumed these tools keep the same files on Linux as on macOS, and
asked for that to be verified per tool. The fallback it names is: support
only the tools verified.

## Findings

Checked on the development machine (Ubuntu 26.10), 2026-11-21. Only the
structure was read: folder and file names and JSON keys, never values.

**Claude Code: installed, verified.**
- `~/.claude/projects/<folder>/<session>.jsonl` holds one JSON object per
  line, one file per session.
  - `<folder>` is the project path with `/` written as `-`.
  - Every line has `cwd`, the project folder, so the real path never has to
    be decoded from the folder name.
  - Assistant lines carry `message.usage`: `input_tokens`, `output_tokens`,
    `cache_creation_input_tokens` and `cache_read_input_tokens`. A response
    can be written more than once while it streams, so usage is counted once
    per `message.id`.
- `~/.claude.json` holds `projects.<absolute path>`, which includes
  `mcpServers` (servers for that project, "local" scope). It holds no
  top-level `mcpServers` on this machine; user-scope servers would go there.
- In a project: `CLAUDE.md`, `.claude/settings.json`, `.claude/agents/`,
  `.claude/commands/`, and `.mcp.json` (`mcpServers`, shared with the team).

**`~/.gemini` here belongs to Google Antigravity** (`antigravity/`,
`antigravity-ide/`, `config/`, `logs/`), not to the Gemini CLI. It has no
`settings.json`, so the Gemini CLI's layout could not be checked.

**Not installed here, so not verified:** OpenAI Codex CLI (`~/.codex`),
Cursor (`~/.cursor`), Windsurf, GitHub Copilot, opencode and Aider.

## Decision

- **Token usage and MCP servers come from Claude Code only** (the fallback).
  The view says which tool each number comes from.
- **Project agent files are recognised for every common tool**, by the file
  names each tool documents. This only checks that a file exists in a
  project folder; nothing is parsed:

  | Tool | Files |
  |---|---|
  | Claude Code | `CLAUDE.md`, `.claude/`, `.mcp.json` |
  | Codex and others | `AGENTS.md` |
  | Gemini CLI | `GEMINI.md` |
  | Cursor | `.cursor/rules/`, `.cursorrules` |
  | GitHub Copilot | `.github/copilot-instructions.md` |
  | Windsurf | `.windsurfrules` |
- **Projects are found:**
  - from Claude Code's logs (`cwd`);
  - by looking for those agent files under the indexed home, when the
    index has them.
- **Tests read sample data** in the verified format (`tests/data/agents/`),
  and the fixture home plants a Claude Code setup the manifest describes.

## Consequences

- Codex, Gemini CLI and Cursor users see their projects and agent files, but
  no token usage or MCP servers from those tools until each is verified on a
  machine that has it (HUMAN: listed in PROGRESS.md). Each one is then a
  parser plus sample data.
- The view never shows prompts or replies: only counts, names and paths.
