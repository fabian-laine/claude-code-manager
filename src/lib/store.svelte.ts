import { getApi, type Api, type ClaudeUsage } from "./api";
import type { Project, ClaudeEvent, RenderedMessage } from "./types";
import { parseClaudeEvent } from "./parseEvent";
import { playFinishChime } from "./notify";

type ProjectState = {
  messages: RenderedMessage[];
  busy: boolean;
  paused: boolean;
  historyLoaded: boolean;
  oldestTs: string | null;
  hasMoreHistory: boolean;
  loadingMore: boolean;
  hasUnseenFinish: boolean;
  contextTokens: number | null;
  contextDismissed: boolean;
  busyStartedAt: number | null;
  autoPaused: boolean;
};

// Warn above this many tokens in the last result's context to suggest /compact.
export const CONTEXT_WARN_THRESHOLD = 100_000;

// Auto-pause a background session that has been busy for this many minutes.
// 0 disables the feature.
const AUTO_PAUSE_AFTER_MS = 5 * 60 * 1000;
const AUTO_PAUSE_CHECK_INTERVAL_MS = 30_000;

// Poll the Claude CLI /usage view this often.
const USAGE_POLL_INTERVAL_MS = 20 * 60 * 1000;

function createStore() {
  let projects = $state<Project[]>([]);
  let activeId = $state<string | null>(null);
  const byProject = $state<Record<string, ProjectState>>({});
  let claudeUsage = $state<ClaudeUsage | null>(null);
  let claudeUsageLoading = $state(false);
  let claudeUsageError = $state<string | null>(null);
  let claudeUsageUpdatedAt = $state<number | null>(null);
  let api: Api | null = null;

  async function ensureApi(): Promise<Api> {
    if (!api) api = await getApi();
    return api;
  }

  function ensureState(id: string): ProjectState {
    if (!byProject[id]) {
      byProject[id] = {
        messages: [],
        busy: false,
        paused: false,
        historyLoaded: false,
        oldestTs: null,
        hasMoreHistory: false,
        loadingMore: false,
        hasUnseenFinish: false,
        contextTokens: null,
        contextDismissed: false,
        busyStartedAt: null,
        autoPaused: false,
      };
    }
    return byProject[id];
  }

  async function refreshProjects() {
    const a = await ensureApi();
    projects = await a.listProjects();
    for (const p of projects) {
      const st = ensureState(p.id);
      // Fresh boot: clear any stale running flags left over from a previous run.
      st.busy = false;
      st.paused = false;
    }
    if (!activeId && projects.length > 0) {
      activeId = projects[0].id;
      loadHistory(projects[0].id);
    }
  }

  async function addProject(name: string, path: string) {
    const a = await ensureApi();
    const p = await a.addProject(name, path);
    projects = [p, ...projects];
    ensureState(p.id);
    activeId = p.id;
    loadHistory(p.id);
  }

  async function deleteProject(id: string) {
    const a = await ensureApi();
    await a.deleteProject(id);
    projects = projects.filter((p) => p.id !== id);
    delete byProject[id];
    if (activeId === id) activeId = projects[0]?.id ?? null;
  }

  async function clearSession(id: string) {
    const a = await ensureApi();
    await a.clearSession(id);
    const p = projects.find((x) => x.id === id);
    if (p) p.last_session_id = null;
    const st = ensureState(id);
    st.messages = [];
    st.oldestTs = null;
    st.hasMoreHistory = false;
    st.historyLoaded = true;
  }

  async function loadHistory(id: string) {
    const a = await ensureApi();
    const st = ensureState(id);
    if (st.historyLoaded) return;
    try {
      const chunk = await a.loadHistoryChunk(id, null, 2);
      const rendered: RenderedMessage[] = [];
      for (const ev of chunk.events) rendered.push(...parseClaudeEvent(ev));
      st.messages = rendered;
      st.oldestTs = chunk.oldest_ts;
      st.hasMoreHistory = chunk.has_more;
      st.historyLoaded = true;
    } catch (e) {
      console.error("loadHistory failed", e);
      st.historyLoaded = true;
    }
  }

  async function loadMoreHistory(id: string): Promise<number> {
    const a = await ensureApi();
    const st = ensureState(id);
    if (!st.hasMoreHistory || st.loadingMore || !st.oldestTs) return 0;
    st.loadingMore = true;
    try {
      const chunk = await a.loadHistoryChunk(id, st.oldestTs, 2);
      const rendered: RenderedMessage[] = [];
      for (const ev of chunk.events) rendered.push(...parseClaudeEvent(ev));
      st.messages = [...rendered, ...st.messages];
      st.oldestTs = chunk.oldest_ts ?? st.oldestTs;
      st.hasMoreHistory = chunk.has_more;
      return rendered.length;
    } catch (e) {
      console.error("loadMoreHistory failed", e);
      return 0;
    } finally {
      st.loadingMore = false;
    }
  }

  async function cancelMessage(id: string) {
    try {
      const a = await ensureApi();
      await a.cancelMessage(id);
    } catch (e) {
      console.error("cancel failed", e);
    }
  }

  async function pauseMessage(id: string) {
    try {
      const a = await ensureApi();
      await a.pauseMessage(id);
    } catch (e) {
      console.error("pause failed", e);
    }
  }

  async function resumeMessage(id: string) {
    try {
      const a = await ensureApi();
      await a.resumeMessage(id);
    } catch (e) {
      console.error("resume failed", e);
    }
  }

  async function resumeWithGuidance(id: string, extraText: string) {
    const text = extraText.trim();
    if (!text) {
      await resumeMessage(id);
      return;
    }
    try {
      const a = await ensureApi();
      await a.cancelMessage(id);
    } catch (e) {
      console.error("cancel on inject failed", e);
    }
    await new Promise((r) => setTimeout(r, 150));
    await sendMessage(id, text);
  }

  async function sendMessage(id: string, prompt: string) {
    const a = await ensureApi();
    const st = ensureState(id);
    st.messages.push({
      type: "user",
      text: prompt,
      id: `u-${Date.now()}`,
    });
    st.busy = true;
    try {
      await a.sendMessage(id, prompt);
    } catch (e) {
      st.messages.push({
        type: "error",
        text: String(e),
        id: `e-${Date.now()}`,
      });
      st.busy = false;
    }
  }

  function handleEvent(ev: ClaudeEvent) {
    const st = ensureState(ev.project_id);
    if (ev.kind === "started") {
      st.busy = true;
      st.autoPaused = false;
      // Only track the background-busy clock when the user isn't on this project.
      st.busyStartedAt = ev.project_id !== activeId ? Date.now() : null;
    } else if (ev.kind === "finished") {
      st.busy = false;
      st.paused = false;
      st.busyStartedAt = null;
      st.autoPaused = false;
      // If the user is currently looking at another project, flag this one
      // so the sidebar shows a green dot, and play a short chime.
      if (ev.project_id !== activeId) {
        st.hasUnseenFinish = true;
        playFinishChime();
      }
      // Refresh context-token estimate so the "getting large" warning fires.
      refreshContextTokens(ev.project_id);
    } else if (ev.kind === "paused") {
      st.paused = true;
    } else if (ev.kind === "resumed") {
      st.paused = false;
      st.autoPaused = false;
    } else if (ev.kind === "cancelled") {
      st.messages.push({
        type: "error",
        text: "Interrupted by user.",
        id: `c-${Date.now()}`,
      });
    } else if (ev.kind === "error") {
      st.messages.push({
        type: "error",
        text: ev.message,
        id: `e-${Date.now()}-${Math.random()}`,
      });
    } else if (ev.kind === "raw") {
      const rendered = parseClaudeEvent(ev.event);
      for (const m of rendered) {
        if (m.type === "assistant_text") {
          const last = st.messages[st.messages.length - 1];
          if (last && last.type === "assistant_text" && last.text === m.text) {
            continue;
          }
        }
        st.messages.push(m);
      }
    }
  }

  async function refreshContextTokens(id: string) {
    try {
      const a = await ensureApi();
      if (!a.sessionStats) return;
      const s = await a.sessionStats(id);
      const st = ensureState(id);
      const tokens =
        s.last_context_tokens ||
        s.input_tokens + s.cache_creation_tokens + s.cache_read_tokens;
      // Reset the dismissed flag when the context shrinks (e.g. after /compact).
      if (st.contextTokens && tokens < st.contextTokens) {
        st.contextDismissed = false;
      }
      st.contextTokens = tokens;
    } catch {
      /* ignore — stats are best-effort */
    }
  }

  function dismissContextWarning(id: string) {
    ensureState(id).contextDismissed = true;
  }

  async function refreshClaudeUsage() {
    if (claudeUsageLoading) return;
    claudeUsageLoading = true;
    claudeUsageError = null;
    try {
      const a = await ensureApi();
      const u = await a.claudeUsage();
      if (u.error) {
        claudeUsageError = u.preview ? `${u.error} — ${u.preview}` : u.error;
      } else {
        claudeUsage = u;
        claudeUsageUpdatedAt = Date.now();
      }
    } catch (e) {
      claudeUsageError = String(e);
    } finally {
      claudeUsageLoading = false;
    }
  }

  let usageTimer: ReturnType<typeof setInterval> | null = null;

  function startUsageWatcher() {
    if (usageTimer) return;
    // First fetch after a short delay so we don't fight the initial UI load.
    setTimeout(() => refreshClaudeUsage(), 10_000);
    usageTimer = setInterval(() => refreshClaudeUsage(), USAGE_POLL_INTERVAL_MS);
  }

  let autoPauseTimer: ReturnType<typeof setInterval> | null = null;

  function startAutoPauseWatcher() {
    if (autoPauseTimer) return;
    autoPauseTimer = setInterval(() => {
      if (AUTO_PAUSE_AFTER_MS <= 0) return;
      const now = Date.now();
      for (const id of Object.keys(byProject)) {
        const st = byProject[id];
        if (!st.busy || st.paused || id === activeId) continue;
        if (st.busyStartedAt == null) continue;
        if (now - st.busyStartedAt >= AUTO_PAUSE_AFTER_MS) {
          st.autoPaused = true;
          pauseMessage(id);
        }
      }
    }, AUTO_PAUSE_CHECK_INTERVAL_MS);
  }

  async function initListener() {
    const a = await ensureApi();
    await a.subscribe(handleEvent);
    startAutoPauseWatcher();
    startUsageWatcher();
  }

  function setActive(id: string) {
    activeId = id;
    const st = ensureState(id);
    // Clear the unseen-finish flag for the project the user just opened.
    st.hasUnseenFinish = false;
    // Cancel the auto-pause clock — this project is now foreground.
    st.busyStartedAt = null;
    loadHistory(id);
    // Pull a fresh context estimate in case we haven't loaded it yet.
    if (st.contextTokens == null) refreshContextTokens(id);
  }

  return {
    get projects() {
      return projects;
    },
    get activeId() {
      return activeId;
    },
    get activeState() {
      return activeId ? ensureState(activeId) : null;
    },
    stateFor: ensureState,
    refreshProjects,
    addProject,
    deleteProject,
    clearSession,
    sendMessage,
    cancelMessage,
    pauseMessage,
    resumeMessage,
    resumeWithGuidance,
    setActive,
    initListener,
    ensureApi,
    loadMoreHistory,
    dismissContextWarning,
    refreshContextTokens,
    get claudeUsage() {
      return claudeUsage;
    },
    get claudeUsageLoading() {
      return claudeUsageLoading;
    },
    get claudeUsageError() {
      return claudeUsageError;
    },
    get claudeUsageUpdatedAt() {
      return claudeUsageUpdatedAt;
    },
    refreshClaudeUsage,
  };
}

export const store = createStore();
