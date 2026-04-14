import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Project, ClaudeEvent, RenderedMessage } from "./types";
import { parseClaudeEvent } from "./parseEvent";

type ProjectState = {
  messages: RenderedMessage[];
  busy: boolean;
  paused: boolean;
  historyLoaded: boolean;
};

function createStore() {
  let projects = $state<Project[]>([]);
  let activeId = $state<string | null>(null);
  const byProject = $state<Record<string, ProjectState>>({});

  function ensureState(id: string): ProjectState {
    if (!byProject[id]) {
      byProject[id] = { messages: [], busy: false, paused: false, historyLoaded: false };
    }
    return byProject[id];
  }

  async function refreshProjects() {
    projects = await invoke<Project[]>("list_projects");
    for (const p of projects) ensureState(p.id);
    if (!activeId && projects.length > 0) {
      activeId = projects[0].id;
      loadHistory(projects[0].id);
    }
  }

  async function addProject(name: string, path: string) {
    const p = await invoke<Project>("add_project", { name, path });
    projects = [p, ...projects];
    ensureState(p.id);
    activeId = p.id;
  }

  async function deleteProject(id: string) {
    await invoke("delete_project", { id });
    projects = projects.filter((p) => p.id !== id);
    delete byProject[id];
    if (activeId === id) activeId = projects[0]?.id ?? null;
  }

  async function loadHistory(id: string) {
    const st = ensureState(id);
    if (st.historyLoaded) return;
    try {
      const events = await invoke<any[]>("load_history", { projectId: id });
      const rendered: RenderedMessage[] = [];
      for (const ev of events) {
        rendered.push(...parseClaudeEvent(ev));
      }
      st.messages = rendered;
      st.historyLoaded = true;
    } catch (e) {
      console.error("loadHistory failed", e);
      st.historyLoaded = true;
    }
  }

  async function cancelMessage(id: string) {
    try {
      await invoke("cancel_message", { projectId: id });
    } catch (e) {
      console.error("cancel failed", e);
    }
  }

  async function pauseMessage(id: string) {
    try {
      await invoke("pause_message", { projectId: id });
    } catch (e) {
      console.error("pause failed", e);
    }
  }

  async function resumeMessage(id: string) {
    try {
      await invoke("resume_message", { projectId: id });
    } catch (e) {
      console.error("resume failed", e);
    }
  }

  /**
   * Resume with additional guidance: kills the currently paused (or running) turn,
   * then starts a new message using the appended text. Claude picks up via --resume,
   * so the new text becomes the next user message in the same conversation.
   */
  async function resumeWithGuidance(id: string, extraText: string) {
    const text = extraText.trim();
    if (!text) {
      await resumeMessage(id);
      return;
    }
    try {
      // Unfreeze then kill so the current turn terminates cleanly.
      await invoke("cancel_message", { projectId: id });
    } catch (e) {
      console.error("cancel on inject failed", e);
    }
    // Give the backend a moment to unregister the old process before spawning the new one.
    await new Promise((r) => setTimeout(r, 150));
    await sendMessage(id, text);
  }

  async function sendMessage(id: string, prompt: string) {
    const st = ensureState(id);
    st.messages.push({
      type: "user",
      text: prompt,
      id: `u-${Date.now()}`,
    });
    st.busy = true;
    try {
      await invoke("send_message", { projectId: id, prompt });
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
    } else if (ev.kind === "finished") {
      st.busy = false;
      st.paused = false;
    } else if (ev.kind === "paused") {
      st.paused = true;
    } else if (ev.kind === "resumed") {
      st.paused = false;
    } else if (ev.kind === "cancelled") {
      st.messages.push({
        type: "error",
        text: "Interrompu par l'utilisateur.",
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
        // Dedup: result event sometimes repeats the last assistant text verbatim.
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

  async function initListener() {
    await listen<ClaudeEvent>("claude-event", (e) => handleEvent(e.payload));
  }

  function setActive(id: string) {
    activeId = id;
    loadHistory(id);
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
    sendMessage,
    cancelMessage,
    pauseMessage,
    resumeMessage,
    resumeWithGuidance,
    setActive,
    initListener,
  };
}

export const store = createStore();
