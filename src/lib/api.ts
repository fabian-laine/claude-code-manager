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

export type Api = {
  listProjects(): Promise<Project[]>;
  addProject(name: string, path: string): Promise<Project>;
  deleteProject(id: string): Promise<void>;
  loadHistory(projectId: string): Promise<any[]>;
  sendMessage(projectId: string, prompt: string): Promise<void>;
  cancelMessage(projectId: string): Promise<void>;
  pauseMessage(projectId: string): Promise<void>;
  resumeMessage(projectId: string): Promise<void>;
  listMcpServers(projectId: string): Promise<{ name: string; status: string; ok: boolean }[]>;
  openFolder(path: string): Promise<void>;

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
    loadHistory: (projectId) => invoke<any[]>("load_history", { projectId }),
    sendMessage: (projectId, prompt) =>
      invoke("send_message", { projectId, prompt }),
    cancelMessage: (projectId) => invoke("cancel_message", { projectId }),
    pauseMessage: (projectId) => invoke("pause_message", { projectId }),
    resumeMessage: (projectId) => invoke("resume_message", { projectId }),
    listMcpServers: (projectId) =>
      invoke("list_mcp_servers", { projectId }),
    openFolder: (path) => invoke("open_folder", { path }),

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
    loadHistory: (projectId) => req(`/api/projects/${projectId}/history`),
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
