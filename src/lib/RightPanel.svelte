<script lang="ts">
  import { store } from "./store.svelte";
  import { isTauri } from "./api";

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
      const api = await store.ensureApi();
      servers = await api.listMcpServers(store.activeId);
      lastLoadedFor = store.activeId;
    } catch (e) {
      errorMsg = String(e);
      servers = [];
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (store.activeId && store.activeId !== lastLoadedFor) {
      refresh();
    }
  });

  async function openFolder() {
    const p = store.projects.find((x) => x.id === store.activeId);
    if (!p) return;
    try {
      const api = await store.ensureApi();
      await api.openFolder(p.path);
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

<aside class="flex flex-col h-full w-full bg-bg-1 overflow-y-auto">
  <header
    class="sticky top-0 bg-bg-1 flex items-center justify-between px-4 py-3.5 border-b border-line z-10"
  >
    <h2 class="text-xs uppercase tracking-widest text-text-2 font-semibold m-0">
      Info
    </h2>
  </header>

  <section class="px-4 py-3.5 border-b border-[#1f1f26]">
    <div class="flex items-center justify-between mb-2.5">
      <h3 class="text-[11px] uppercase tracking-wider text-text-3 font-semibold m-0">
        MCP servers
      </h3>
      <button
        onclick={refresh}
        disabled={loading}
        title="Refresh"
        aria-label="Refresh"
        class="bg-transparent border border-line-2 text-text-2 w-5.5 h-5.5 rounded cursor-pointer text-xs hover:bg-bg-2 hover:text-text-0 disabled:opacity-40"
      >
        {loading ? "…" : "↻"}
      </button>
    </div>
    {#if errorMsg}
      <div class="text-[#f87171] text-xs break-words">{errorMsg}</div>
    {:else if loading && servers.length === 0}
      <div class="text-text-3 text-xs italic">Loading…</div>
    {:else if servers.length === 0}
      <div class="text-text-3 text-xs italic">No MCP servers detected.</div>
    {:else}
      <ul class="list-none p-0 m-0">
        {#each servers as s}
          <li class="flex gap-2.5 items-center py-1.5">
            <span
              class="shrink-0 w-2 h-2 rounded-full {s.ok
                ? 'bg-ok'
                : 'bg-danger'}"
            ></span>
            <div class="flex-1 min-w-0">
              <div class="text-[13px] text-text-0 font-medium">{s.name}</div>
              <div class="text-[11px] text-text-3 truncate">{s.status}</div>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="px-4 py-3.5 border-b border-[#1f1f26]">
    <h3 class="text-[11px] uppercase tracking-wider text-text-3 font-semibold m-0 mb-2.5">
      Actions
    </h3>
    <div class="flex flex-col gap-1.5">
      {#if isTauri}
        <button
          onclick={openFolder}
          disabled={!activeProject}
          class="flex items-center gap-2.5 bg-bg-2 border border-line text-text-1 px-3 py-2 rounded-md cursor-pointer text-[13px] text-left hover:bg-[#24242e] hover:text-text-0 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          <span class="w-4 text-center">📁</span> Open folder
        </button>
      {/if}
      <button
        onclick={copySessionId}
        disabled={!activeProject?.last_session_id}
        class="flex items-center gap-2.5 bg-bg-2 border border-line text-text-1 px-3 py-2 rounded-md cursor-pointer text-[13px] text-left hover:bg-[#24242e] hover:text-text-0 disabled:opacity-40 disabled:cursor-not-allowed"
      >
        <span class="w-4 text-center">⎘</span> Copy session ID
      </button>
    </div>
  </section>

  {#if activeProject}
    <section class="px-4 py-3.5">
      <h3
        class="text-[11px] uppercase tracking-wider text-text-3 font-semibold m-0 mb-2.5"
      >
        Project
      </h3>
      <div class="mb-2">
        <div class="text-[10px] uppercase text-text-3 tracking-wide">Path</div>
        <div class="text-xs text-text-1 font-mono break-all">
          {activeProject.path}
        </div>
      </div>
      <div>
        <div class="text-[10px] uppercase text-text-3 tracking-wide">Session</div>
        <div class="text-xs text-text-1 font-mono break-all">
          {activeProject.last_session_id ?? "—"}
        </div>
      </div>
    </section>
  {/if}
</aside>
