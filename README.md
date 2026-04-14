<div align="center">
  <img src="Icon.png" alt="Claude Code Manager" width="128" height="128" />
  <h1>Claude Code Manager</h1>
  <p>A dark-themed desktop app to manage multiple <a href="https://claude.com/claude-code">Claude Code</a> sessions across your projects — from a single window.</p>
</div>

---

## What is it?

Claude Code Manager is a lightweight Linux desktop app (built with Tauri + Rust + Svelte 5) that lets you drive the official `claude` CLI from a graphical interface. Think of it as a project switcher and chat UI sitting on top of Claude Code — you register your codebases once, then chat with Claude in each of them without juggling terminals.

Under the hood, the app spawns real `claude` processes with `--output-format stream-json` and parses the event stream, so everything the CLI supports (tools, MCP servers, skills, hooks, session resume) works out of the box.

## Features

- **Multi-project sidebar** — register any number of local codebases and switch between them instantly.
- **Parallel sessions** — run Claude in several projects at the same time; each has its own running state.
- **Session resume** — sessions are persisted by Claude Code itself (`~/.claude/projects/*.jsonl`), so closing the app or restarting it never loses context. The next message automatically uses `--resume`.
- **CLI-style rendering** — tool calls (Read, Edit, Write, Bash, Grep, Glob, TodoWrite, Task, WebFetch…) are rendered with icons, collapsible details, real diff view for `Edit`, todo checklists, and syntax-highlighted code blocks.
- **Full Markdown** in assistant responses (GFM tables, code blocks with highlight.js, headings, lists, quotes, links).
- **Pause with added guidance** — mid-turn, freeze the entire Claude process tree with `SIGSTOP`, optionally type extra instructions, then resume (either continue as-is with `SIGCONT`, or abort the current turn and inject your new message into the same conversation via `--resume`).
- **Right panel** — collapsible panel showing configured MCP servers (live from `claude mcp list`), quick actions (open project folder, copy session id), and project metadata.
- **Skip permissions by default** — runs Claude with `--dangerously-skip-permissions` so you never get blocked by prompts.
- **Native dark theme** built for long coding sessions.

## Screenshots

<!-- Add screenshots here -->

## Requirements

- **Linux** (Fedora, Ubuntu, Debian, Arch, openSUSE…)
- **[Claude Code CLI](https://docs.claude.com/en/docs/claude-code/quickstart)** installed and authenticated (`claude` must be in your `PATH`)
- A desktop environment with `webkit2gtk` available (installed by most DEs)

## Installation

### Fedora / RHEL / openSUSE (RPM)

Download the `.rpm` from the [latest release](https://github.com/fabian-laine/claude-code-manager/releases/latest), then:

```bash
sudo dnf install ./claude-code-manager-*.x86_64.rpm
```

### Debian / Ubuntu (DEB)

```bash
sudo apt install ./claude-code-manager_*_amd64.deb
```

### Any distro (AppImage)

```bash
chmod +x claude-code-manager_*_amd64.AppImage
./claude-code-manager_*_amd64.AppImage
```

### Arch Linux

No official package yet — build from source (see below) or use the AppImage.

## Build from source

```bash
# 1. System dependencies (Fedora example — adapt for your distro)
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
    libappindicator-gtk3-devel librsvg2-devel patchelf

# 2. Rust + Node
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# (Node 20+ with pnpm)

# 3. Build
git clone https://github.com/fabian-laine/claude-code-manager.git
cd claude-code-manager
pnpm install
pnpm tauri build --bundles rpm deb appimage
```

Artifacts land in `src-tauri/target/release/bundle/`.

## Troubleshooting

**The app crashes on launch with a Wayland protocol error.** This is a known webkit2gtk issue on recent Fedora/GNOME. The installed `.desktop` file already sets the workaround env vars, but if you launch the binary directly:

```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 claude-code-manager
```

**"claude: command not found"** — install and authenticate [Claude Code](https://docs.claude.com/en/docs/claude-code/quickstart) first.

## Architecture

```
┌───────────────┐       ┌────────────────────────┐      ┌──────────────┐
│  Svelte 5 UI  │◄─────►│   Rust / Tauri backend │◄────►│  claude CLI  │
│  (SvelteKit)  │ events│  session manager       │stdio │  subprocess  │
└───────────────┘       │  SQLite (projects)     │      └──────────────┘
                        └────────────────────────┘             │
                                                               ▼
                                                ~/.claude/projects/*.jsonl
                                                (transcripts — source of truth)
```

- **Backend (Rust):** spawns `claude` per message, streams stdout events, manages a process registry for pause/resume/cancel, stores the list of projects in a local SQLite file.
- **Frontend (Svelte 5 runes):** reactive per-project state, renders the event stream into CLI-style messages.
- **Conversations** are **not** duplicated in our database — they are read directly from Claude Code's own JSONL transcripts, so there's zero risk of drift.

## Roadmap

- [ ] Per-project system prompt / model override
- [ ] Slash command palette
- [ ] Search across past sessions
- [ ] Windows / macOS builds
- [ ] Arch AUR package

## License

MIT
