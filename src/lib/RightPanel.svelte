<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { store } from "./store.svelte";

  let { collapsed = $bindable() }: { collapsed: boolean } = $props();

  type McpServer = { name: string; status: string; ok: boolean };
  let servers = $state<McpServer[]>([]);
  let loading = $state(false);
  let errorMsg = $state<string | null>(null);
  let lastLoadedFor = $state<string | null>(null);

  async function refresh() {
    if (!store.activeId) return;
    loading = true;
    errorMsg = null;
    try {
      servers = await invoke<McpServer[]>("list_mcp_servers", {
        projectId: store.activeId,
      });
      lastLoadedFor = store.activeId;
    } catch (e) {
      errorMsg = String(e);
      servers = [];
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (!collapsed && store.activeId && store.activeId !== lastLoadedFor) {
      refresh();
    }
  });

  async function openFolder() {
    const p = store.projects.find((x) => x.id === store.activeId);
    if (!p) return;
    try {
      await invoke("open_folder", { path: p.path });
    } catch (e) {
      console.error("open folder failed", e);
    }
  }

  function copySessionId() {
    const p = store.projects.find((x) => x.id === store.activeId);
    if (!p?.last_session_id) return;
    navigator.clipboard.writeText(p.last_session_id);
  }

  const activeProject = $derived(
    store.projects.find((p) => p.id === store.activeId) ?? null,
  );
</script>

{#if collapsed}
  <button class="rail" onclick={() => (collapsed = false)} title="Ouvrir le panneau">
    ◀
  </button>
{:else}
  <aside class="panel">
    <header>
      <h2>Infos</h2>
      <button class="close" onclick={() => (collapsed = true)} title="Masquer">▶</button>
    </header>

    <section>
      <div class="section-head">
        <h3>Serveurs MCP</h3>
        <button class="refresh" onclick={refresh} disabled={loading} title="Rafraîchir">
          {loading ? "…" : "↻"}
        </button>
      </div>
      {#if errorMsg}
        <div class="err">{errorMsg}</div>
      {:else if loading && servers.length === 0}
        <div class="muted">Chargement…</div>
      {:else if servers.length === 0}
        <div class="muted">Aucun serveur MCP détecté.</div>
      {:else}
        <ul class="servers">
          {#each servers as s}
            <li>
              <span class="dot" class:ok={s.ok}></span>
              <div class="info">
                <div class="nm">{s.name}</div>
                <div class="st">{s.status}</div>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <section>
      <h3>Actions</h3>
      <div class="actions">
        <button onclick={openFolder} disabled={!activeProject}>
          <span class="ico">📁</span> Ouvrir le dossier
        </button>
        <button onclick={copySessionId} disabled={!activeProject?.last_session_id}>
          <span class="ico">⎘</span> Copier l'ID de session
        </button>
      </div>
    </section>

    {#if activeProject}
      <section>
        <h3>Projet</h3>
        <div class="kv">
          <div class="k">Chemin</div>
          <div class="v mono">{activeProject.path}</div>
        </div>
        <div class="kv">
          <div class="k">Session</div>
          <div class="v mono">{activeProject.last_session_id ?? "—"}</div>
        </div>
      </section>
    {/if}
  </aside>
{/if}

<style>
  .rail {
    width: 24px;
    background: #141418;
    border-left: 1px solid #26262d;
    color: #6a6a75;
    border-top: none;
    border-right: none;
    border-bottom: none;
    cursor: pointer;
    font-size: 12px;
  }
  .rail:hover {
    background: #1b1b23;
    color: #e8e8ee;
  }
  .panel {
    width: 300px;
    min-width: 300px;
    background: #141418;
    border-left: 1px solid #26262d;
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow-y: auto;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    border-bottom: 1px solid #26262d;
    position: sticky;
    top: 0;
    background: #141418;
    z-index: 1;
  }
  h2 {
    font-size: 13px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: #8a8a96;
    margin: 0;
  }
  .close {
    background: transparent;
    border: none;
    color: #6a6a75;
    cursor: pointer;
    font-size: 13px;
  }
  .close:hover {
    color: #e8e8ee;
  }
  section {
    padding: 14px 16px;
    border-bottom: 1px solid #1f1f26;
  }
  section:last-child {
    border-bottom: none;
  }
  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #6a6a75;
    margin: 0 0 10px 0;
    font-weight: 600;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }
  .section-head h3 {
    margin: 0;
  }
  .refresh {
    background: transparent;
    border: 1px solid #2a2a33;
    color: #9a9aa5;
    width: 22px;
    height: 22px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
    padding: 0;
  }
  .refresh:hover:not(:disabled) {
    background: #1e1e25;
    color: #e8e8ee;
  }
  .muted {
    color: #6a6a75;
    font-size: 12px;
    font-style: italic;
  }
  .err {
    color: #f87171;
    font-size: 12px;
    word-break: break-word;
  }
  .servers {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .servers li {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 6px 0;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #b34545;
    flex-shrink: 0;
  }
  .dot.ok {
    background: #7aa870;
  }
  .info {
    flex: 1;
    min-width: 0;
  }
  .nm {
    font-size: 13px;
    color: #e8e8ee;
    font-weight: 500;
  }
  .st {
    font-size: 11px;
    color: #6a6a75;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .actions button {
    display: flex;
    align-items: center;
    gap: 10px;
    background: #1a1a22;
    border: 1px solid #26262d;
    color: #c8c8d2;
    padding: 8px 12px;
    border-radius: 6px;
    cursor: pointer;
    font-family: inherit;
    font-size: 13px;
    text-align: left;
  }
  .actions button:hover:not(:disabled) {
    background: #24242e;
    color: #e8e8ee;
  }
  .actions button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .ico {
    width: 16px;
    text-align: center;
  }
  .kv {
    margin-bottom: 8px;
  }
  .k {
    font-size: 10px;
    text-transform: uppercase;
    color: #6a6a75;
    letter-spacing: 0.06em;
  }
  .v {
    font-size: 12px;
    color: #c8c8d2;
    word-break: break-all;
  }
  .mono {
    font-family: "JetBrains Mono", ui-monospace, monospace;
  }
</style>
