import type { Api } from "./api";
import { store } from "./store.svelte";

export type FileTab = { label: string; path: string; readonly?: boolean };

export type SlashCommandContext = {
  activeId: string | null;
  api: Api;
  openSettings: () => void;
  toggleRightPanel: () => void;
  showHelp: () => void;
  showDiff: () => void;
  showFileEditor: (title: string, tabs: FileTab[]) => void;
  showFileBrowser: (title: string, dirs: { label: string; path: string }[]) => void;
  showCost: () => void;
  showContext: () => void;
  showStats: () => void;
  showModelPicker: () => void;
  showEffortPicker: () => void;
  showAddDir: () => void;
};

export type SlashCommand = {
  name: string;
  description: string;
  argHint?: string;
  run: (ctx: SlashCommandContext, args: string) => Promise<void> | void;
};

const REPO_URL = "https://github.com/fabian-laine/claude-code-manager";

async function openUrl(url: string): Promise<void> {
  try {
    const { openUrl: tauriOpen } = await import("@tauri-apps/plugin-opener");
    await tauriOpen(url);
  } catch {
    if (typeof window !== "undefined") window.open(url, "_blank");
  }
}

export const slashCommands: SlashCommand[] = [
  {
    name: "clear",
    description: "Start a new Claude session (keeps the prior transcript on disk)",
    async run(ctx) {
      if (!ctx.activeId) return;
      await store.clearSession(ctx.activeId);
    },
  },
  {
    name: "copy",
    argHint: "[N]",
    description: "Copy the Nth most recent assistant message (default: last)",
    async run(ctx, args) {
      const n = Math.max(1, parseInt(args.trim() || "1", 10));
      const messages = ctx.activeId
        ? store.stateFor(ctx.activeId).messages
        : [];
      const assistant = messages.filter((m) => m.type === "assistant_text");
      const target = assistant[assistant.length - n] as
        | { type: "assistant_text"; text: string }
        | undefined;
      if (!target) return;
      try {
        await navigator.clipboard.writeText(target.text);
      } catch {
        /* ignore */
      }
    },
  },
  {
    name: "copy-session",
    description: "Copy the current session ID to clipboard",
    async run(ctx) {
      const p = store.projects.find((x) => x.id === ctx.activeId);
      if (!p?.last_session_id) return;
      try {
        await navigator.clipboard.writeText(p.last_session_id);
      } catch {
        /* ignore */
      }
    },
  },
  {
    name: "diff",
    description: "Show uncommitted git diff in a modal",
    run(ctx) {
      ctx.showDiff();
    },
  },
  {
    name: "exit",
    description: "Close the application",
    async run() {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        await getCurrentWindow().close();
      } catch {
        if (typeof window !== "undefined") window.close();
      }
    },
  },
  {
    name: "feedback",
    description: "Open the GitHub issue tracker to report a bug or suggest a feature",
    run() {
      openUrl(`${REPO_URL}/issues`);
    },
  },
  {
    name: "help",
    description: "Show all available slash commands",
    run(ctx) {
      ctx.showHelp();
    },
  },
  {
    name: "mcp",
    description: "Show the Info panel with MCP servers",
    run(ctx) {
      ctx.toggleRightPanel();
    },
  },
  {
    name: "open",
    description: "Open the project folder in your file manager",
    async run(ctx) {
      if (!ctx.activeId) return;
      const p = store.projects.find((x) => x.id === ctx.activeId);
      if (!p) return;
      await ctx.api.openFolder(p.path);
    },
  },
  {
    name: "release-notes",
    description: "Open the GitHub releases page",
    run() {
      openUrl(`${REPO_URL}/releases`);
    },
  },
  {
    name: "settings",
    description: "Open the Settings modal",
    run(ctx) {
      ctx.openSettings();
    },
  },
  {
    name: "memory",
    description: "Edit CLAUDE.md for the project and your user memory",
    run(ctx) {
      const project = store.projects.find((p) => p.id === ctx.activeId);
      const tabs: FileTab[] = [];
      if (project) {
        tabs.push({ label: "Project CLAUDE.md", path: `${project.path}/CLAUDE.md` });
      }
      tabs.push({ label: "User CLAUDE.md", path: "~/.claude/CLAUDE.md" });
      ctx.showFileEditor("Memory", tabs);
    },
  },
  {
    name: "agents",
    description: "Browse agent definitions (project + user)",
    run(ctx) {
      const project = store.projects.find((p) => p.id === ctx.activeId);
      const dirs = [] as { label: string; path: string }[];
      if (project) dirs.push({ label: "Project", path: `${project.path}/.claude/agents` });
      dirs.push({ label: "User", path: "~/.claude/agents" });
      ctx.showFileBrowser("Agents", dirs);
    },
  },
  {
    name: "skills",
    description: "Browse skill definitions (project + user)",
    run(ctx) {
      const project = store.projects.find((p) => p.id === ctx.activeId);
      const dirs = [] as { label: string; path: string }[];
      if (project) dirs.push({ label: "Project", path: `${project.path}/.claude/skills` });
      dirs.push({ label: "User", path: "~/.claude/skills" });
      ctx.showFileBrowser("Skills", dirs);
    },
  },
  {
    name: "hooks",
    description: "View hook configurations from .claude/settings.json",
    run(ctx) {
      const project = store.projects.find((p) => p.id === ctx.activeId);
      const tabs: FileTab[] = [];
      if (project)
        tabs.push({
          label: "Project",
          path: `${project.path}/.claude/settings.json`,
          readonly: true,
        });
      tabs.push({ label: "User", path: "~/.claude/settings.json", readonly: true });
      ctx.showFileEditor("Hooks (settings.json)", tabs);
    },
  },
  {
    name: "permissions",
    description: "Edit .claude/settings.json (permissions and more)",
    run(ctx) {
      const project = store.projects.find((p) => p.id === ctx.activeId);
      const tabs: FileTab[] = [];
      if (project)
        tabs.push({ label: "Project", path: `${project.path}/.claude/settings.json` });
      tabs.push({ label: "User", path: "~/.claude/settings.json" });
      ctx.showFileEditor("Permissions / settings.json", tabs);
    },
  },
  {
    name: "keybindings",
    description: "Edit ~/.claude/keybindings.json",
    run(ctx) {
      ctx.showFileEditor("Keybindings", [
        { label: "Keybindings", path: "~/.claude/keybindings.json" },
      ]);
    },
  },
  {
    name: "init",
    description: "Analyze this codebase and generate a CLAUDE.md",
    async run(ctx) {
      if (!ctx.activeId) return;
      await store.sendMessage(
        ctx.activeId,
        "Please analyze this codebase and create a CLAUDE.md file at the project root. Include: a brief overview, key directories, primary build/test commands, notable conventions, and anything else a new contributor (human or AI) should know before editing code here. Keep it concise and factual — do not invent features.",
      );
    },
  },
  {
    name: "compact",
    argHint: "[focus]",
    description:
      "Summarize the conversation and start a new session with the summary as context",
    async run(ctx, args) {
      if (!ctx.activeId) return;
      const focus = args.trim() ? ` Focus on: ${args.trim()}.` : "";
      await store.sendMessage(
        ctx.activeId,
        `Please produce a concise recap of our conversation so far that a future Claude session can use as context: decisions made, current state, open work, and any key file paths or invariants.${focus} Keep it under 300 words.`,
      );
    },
  },
  {
    name: "recap",
    description: "Ask Claude for a one-line summary of this session",
    async run(ctx) {
      if (!ctx.activeId) return;
      await store.sendMessage(
        ctx.activeId,
        "Summarize the session so far in one short line.",
      );
    },
  },
  {
    name: "review",
    description: "Ask Claude to review the current branch's changes",
    async run(ctx) {
      if (!ctx.activeId) return;
      await store.sendMessage(
        ctx.activeId,
        "Please review the uncommitted and recent committed changes on this branch (git diff + git log). Flag code quality issues, bugs, missing tests, and unclear naming. Be specific and reference file:line when relevant.",
      );
    },
  },
  {
    name: "security-review",
    description: "Ask Claude to audit pending changes for security issues",
    async run(ctx) {
      if (!ctx.activeId) return;
      await store.sendMessage(
        ctx.activeId,
        "Please perform a security review of the pending changes on this branch. Look specifically for: injection vulnerabilities (SQL, command, XSS), broken authentication/authorization, sensitive data exposure, insecure deserialization, SSRF, and the OWASP Top 10. Report with severity and the file:line where each issue lives.",
      );
    },
  },
  {
    name: "cost",
    description: "Show token usage and estimated cost for the current session",
    run(ctx) {
      ctx.showCost();
    },
  },
  {
    name: "context",
    description: "Show how much of the model's context window is in use",
    run(ctx) {
      ctx.showContext();
    },
  },
  {
    name: "stats",
    description: "Show cross-session usage statistics",
    run(ctx) {
      ctx.showStats();
    },
  },
  {
    name: "model",
    argHint: "[model]",
    description: "Pick the Claude model used for the next message",
    async run(ctx, args) {
      if (args.trim()) {
        if (!ctx.activeId) return;
        await ctx.api.setProjectFlag?.(ctx.activeId, "model", args.trim());
      } else {
        ctx.showModelPicker();
      }
    },
  },
  {
    name: "effort",
    argHint: "[level]",
    description: "Set model effort level (low/medium/high/xhigh/auto)",
    async run(ctx, args) {
      if (args.trim()) {
        if (!ctx.activeId) return;
        const v = args.trim().toLowerCase();
        await ctx.api.setProjectFlag?.(ctx.activeId, "effort", v === "auto" ? "" : v);
      } else {
        ctx.showEffortPicker();
      }
    },
  },
  {
    name: "add-dir",
    argHint: "<path>",
    description: "Add an extra working directory for Claude to access",
    async run(ctx, args) {
      const path = args.trim();
      if (!ctx.activeId) return;
      if (path) {
        await ctx.api.addProjectDir?.(ctx.activeId, path);
      } else {
        ctx.showAddDir();
      }
    },
  },
];

export function matchCommands(query: string): SlashCommand[] {
  const q = query.toLowerCase();
  if (!q) return slashCommands;
  return slashCommands.filter((c) => c.name.toLowerCase().startsWith(q));
}
