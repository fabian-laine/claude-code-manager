import type { Project, ClaudeEvent } from "./types";

export type RemoteStatus = {
  enabled: boolean;
  port: number;
  token: string;
  https: boolean;
  cert_ready: boolean;
  cert_hostname: string | null;
};

export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const TOKEN_KEY = "ccm.token";

export function getWebToken(): string | null {
  if (typeof window === "undefined") return null;
  return window.localStorage.getItem(TOKEN_KEY);
}

export function setWebToken(token: string): void {
  window.localStorage.setItem(TOKEN_KEY, token);
}

export function clearWebToken(): void {
  window.localStorage.removeItem(TOKEN_KEY);
}

/* -------------------------------------------------------------------------- */
/*  Unified API                                                                */
/* -------------------------------------------------------------------------- */

export type HistoryChunk = {
  events: any[];
  oldest_ts: string | null;
  has_more: boolean;
  session_id: string | null;
};

export type SessionStats = {
  input_tokens: number;
  output_tokens: number;
  cache_creation_tokens: number;
  cache_read_tokens: number;
  last_context_tokens: number;
  model: string | null;
  user_messages: number;
  assistant_messages: number;
  session_id: string | null;
};

export type ProjectStats = {
  name: string;
  sessions: number;
  input_tokens: number;
  output_tokens: number;
  cache_creation_tokens: number;
  cache_read_tokens: number;
};

export type GlobalStats = {
  total_sessions: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_cache_creation_tokens: number;
  total_cache_read_tokens: number;
  by_project: ProjectStats[];
};

export type ProjectFlags = {
  model: string | null;
  effort: string | null;
  add_dirs: string[];
};

export type Api = {
  listProjects(): Promise<Project[]>;
  addProject(name: string, path: string): Promise<Project>;
  deleteProject(id: string): Promise<void>;
  clearSession(projectId: string): Promise<void>;
  saveAttachment(
    projectId: string,
    filename: string,
    data: Uint8Array,
  ): Promise<string>;
  copyAttachment?(projectId: string, srcPath: string): Promise<string>;
  sttStatus(): Promise<{
    available: boolean;
    model_ready: boolean;
    model_path: string;
  }>;
  sttDownloadModel(): Promise<void>;
  sttTranscribe(wav: Uint8Array, lang?: string): Promise<string>;
  loadHistoryChunk(
    projectId: string,
    beforeTs?: string | null,
    hours?: number,
  ): Promise<HistoryChunk>;
  sendMessage(projectId: string, prompt: string): Promise<void>;
  cancelMessage(projectId: string): Promise<void>;
  pauseMessage(projectId: string): Promise<void>;
  resumeMessage(projectId: string): Promise<void>;
  listMcpServers(projectId: string): Promise<{ name: string; status: string; ok: boolean }[]>;
  openFolder(path: string): Promise<void>;
  gitDiff(projectId: string): Promise<string>;
  fsRead?(path: string): Promise<string>;
  fsWrite?(path: string, content: string): Promise<void>;
  fsList?(path: string): Promise<{ name: string; path: string; is_dir: boolean }[]>;
  sessionStats?(projectId: string): Promise<SessionStats>;
  globalStats?(): Promise<GlobalStats>;
  getProjectFlags?(projectId: string): Promise<ProjectFlags>;
  setProjectFlag?(projectId: string, name: string, value: string): Promise<void>;
  addProjectDir?(projectId: string, path: string): Promise<string[]>;
  removeProjectDir?(projectId: string, path: string): Promise<string[]>;

  // Remote control (desktop only)
  remoteStatus?(): Promise<RemoteStatus>;
  remoteStart?(port?: number, https?: boolean): Promise<RemoteStatus>;
  remoteStop?(): Promise<void>;
  remoteRotateToken?(): Promise<string>;
  remoteUrls?(): Promise<{ hostname: string | null; tailscale: string | null }>;
  remoteGenerateTlsCert?(hostname: string): Promise<{ hostname: string; cert_path: string; key_path: string }>;

  // Event subscription
  subscribe(cb: (ev: ClaudeEvent) => void): Promise<() => void>;
};

/* -------------------------------------------------------------------------- */
/*  Tauri transport                                                            */
/* -------------------------------------------------------------------------- */

async function makeTauriApi(): Promise<Api> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");

  return {
    listProjects: () => invoke<Project[]>("list_projects"),
    addProject: (name, path) => invoke<Project>("add_project", { name, path }),
    deleteProject: (id) => invoke("delete_project", { id }),
    clearSession: (projectId) => invoke("clear_session", { projectId }),
    saveAttachment: (projectId, filename, data) =>
      invoke<string>("save_attachment", {
        projectId,
        filename,
        data: Array.from(data),
      }),
    copyAttachment: (projectId, srcPath) =>
      invoke<string>("copy_attachment", { projectId, srcPath }),
    sttStatus: () => invoke("stt_status"),
    sttDownloadModel: () => invoke("stt_download_model"),
    sttTranscribe: (wav, lang) =>
      invoke<string>("stt_transcribe", {
        wav: Array.from(wav),
        lang: lang ?? null,
      }),
    loadHistoryChunk: (projectId, beforeTs, hours) =>
      invoke<HistoryChunk>("load_history_chunk", {
        projectId,
        beforeTs: beforeTs ?? null,
        hours: hours ?? 2,
      }),
    sendMessage: (projectId, prompt) =>
      invoke("send_message", { projectId, prompt }),
    cancelMessage: (projectId) => invoke("cancel_message", { projectId }),
    pauseMessage: (projectId) => invoke("pause_message", { projectId }),
    resumeMessage: (projectId) => invoke("resume_message", { projectId }),
    listMcpServers: (projectId) =>
      invoke("list_mcp_servers", { projectId }),
    openFolder: (path) => invoke("open_folder", { path }),
    gitDiff: (projectId) => invoke<string>("git_diff", { projectId }),
    fsRead: (path) => invoke<string>("fs_read", { path }),
    fsWrite: (path, content) => invoke("fs_write", { path, content }),
    fsList: (path) => invoke("fs_list", { path }),
    sessionStats: (projectId) => invoke<SessionStats>("session_stats", { projectId }),
    globalStats: () => invoke<GlobalStats>("global_stats"),
    getProjectFlags: (projectId) => invoke<ProjectFlags>("get_project_flags", { projectId }),
    setProjectFlag: (projectId, name, value) =>
      invoke("set_project_flag", { projectId, name, value }),
    addProjectDir: (projectId, path) =>
      invoke<string[]>("add_project_dir", { projectId, path }),
    removeProjectDir: (projectId, path) =>
      invoke<string[]>("remove_project_dir", { projectId, path }),

    remoteStatus: () => invoke("remote_status"),
    remoteStart: (port, https) => invoke("remote_start", { port, https }),
    remoteStop: () => invoke("remote_stop"),
    remoteRotateToken: () => invoke("remote_rotate_token"),
    remoteUrls: () => invoke("remote_urls"),
    remoteGenerateTlsCert: (hostname) =>
      invoke("remote_generate_tls_cert", { hostname }),

    async subscribe(cb) {
      const unlisten = await listen<ClaudeEvent>("claude-event", (e) =>
        cb(e.payload),
      );
      return unlisten;
    },
  };
}

/* -------------------------------------------------------------------------- */
/*  HTTP / WebSocket transport (web, phone, etc.)                              */
/* -------------------------------------------------------------------------- */

function makeHttpApi(): Api {
  const base = window.location.origin;

  function headers(): HeadersInit {
    const token = getWebToken();
    return {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    };
  }

  async function req<T>(path: string, init: RequestInit = {}): Promise<T> {
    const res = await fetch(`${base}${path}`, {
      ...init,
      headers: { ...headers(), ...(init.headers ?? {}) },
    });
    if (res.status === 401) {
      clearWebToken();
      throw new Error("Unauthorized — please enter a valid access token.");
    }
    if (!res.ok) {
      throw new Error(`${res.status} ${res.statusText}`);
    }
    const ct = res.headers.get("content-type") ?? "";
    if (ct.includes("application/json")) return (await res.json()) as T;
    return undefined as T;
  }

  return {
    listProjects: () => req("/api/projects"),
    addProject: (name, path) =>
      req("/api/projects", {
        method: "POST",
        body: JSON.stringify({ name, path }),
      }),
    deleteProject: (id) =>
      req(`/api/projects/${id}`, { method: "DELETE" }),
    clearSession: (projectId) =>
      req(`/api/projects/${projectId}/clear`, { method: "POST" }),
    async saveAttachment(projectId, filename, data) {
      const token = getWebToken();
      const res = await fetch(`${base}/api/projects/${projectId}/attachments`, {
        method: "POST",
        headers: {
          "Content-Type": "application/octet-stream",
          "X-Filename": encodeURIComponent(filename),
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        body: data,
      });
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      const json = (await res.json()) as { ref_path: string };
      return json.ref_path;
    },
    sttStatus: () => req("/api/stt/status"),
    sttDownloadModel: () =>
      req("/api/stt/download", { method: "POST" }),
    async sttTranscribe(wav, lang) {
      const token = getWebToken();
      const qs = lang ? `?lang=${encodeURIComponent(lang)}` : "";
      const res = await fetch(`${base}/api/stt/transcribe${qs}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/octet-stream",
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        body: wav,
      });
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      const json = (await res.json()) as { text: string };
      return json.text;
    },
    loadHistoryChunk: (projectId, beforeTs, hours) => {
      const params = new URLSearchParams();
      if (beforeTs) params.set("before_ts", beforeTs);
      if (hours != null) params.set("hours", String(hours));
      const qs = params.toString();
      const suffix = qs ? `?${qs}` : "";
      return req<HistoryChunk>(`/api/projects/${projectId}/history${suffix}`);
    },
    sendMessage: (projectId, prompt) =>
      req(`/api/projects/${projectId}/messages`, {
        method: "POST",
        body: JSON.stringify({ prompt }),
      }),
    cancelMessage: (projectId) =>
      req(`/api/projects/${projectId}/cancel`, { method: "POST" }),
    pauseMessage: (projectId) =>
      req(`/api/projects/${projectId}/pause`, { method: "POST" }),
    resumeMessage: (projectId) =>
      req(`/api/projects/${projectId}/resume`, { method: "POST" }),
    listMcpServers: (projectId) => req(`/api/projects/${projectId}/mcp`),
    openFolder: async () => {
      /* noop in web mode */
    },
    async gitDiff(projectId) {
      const token = getWebToken();
      const res = await fetch(`${base}/api/projects/${projectId}/diff`, {
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      });
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      return res.text();
    },

    async subscribe(cb) {
      let ws: WebSocket | null = null;
      let closed = false;
      let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

      const connect = () => {
        if (closed) return;
        const token = getWebToken() ?? "";
        const proto = location.protocol === "https:" ? "wss:" : "ws:";
        const url = `${proto}//${location.host}/api/events?token=${encodeURIComponent(token)}`;
        ws = new WebSocket(url);
        ws.onmessage = (e) => {
          try {
            cb(JSON.parse(e.data));
          } catch {}
        };
        ws.onclose = () => {
          if (closed) return;
          reconnectTimer = setTimeout(connect, 1500);
        };
        ws.onerror = () => {
          ws?.close();
        };
      };
      connect();

      return () => {
        closed = true;
        if (reconnectTimer) clearTimeout(reconnectTimer);
        ws?.close();
      };
    },
  };
}

/* -------------------------------------------------------------------------- */

let _api: Api | null = null;

export async function getApi(): Promise<Api> {
  if (_api) return _api;
  _api = isTauri ? await makeTauriApi() : makeHttpApi();
  return _api;
}
