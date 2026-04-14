<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { store } from "./store.svelte";

  async function addProject() {
    const picked = await open({ directory: true, multiple: false });
    if (!picked || typeof picked !== "string") return;
    const name = picked.split("/").filter(Boolean).pop() ?? picked;
    await store.addProject(name, picked);
  }

  async function removeProject(e: MouseEvent, id: string) {
    e.stopPropagation();
    if (!confirm("Supprimer ce projet ?")) return;
    await store.deleteProject(id);
  }
</script>

<aside class="sidebar">
  <header>
    <h1>Projets</h1>
    <button class="add-btn" onclick={addProject} title="Ajouter un projet">+</button>
  </header>
  <ul>
    {#each store.projects as p (p.id)}
      {@const st = store.stateFor(p.id)}
      <li
        class:active={store.activeId === p.id}
        onclick={() => store.setActive(p.id)}
        role="button"
        tabindex="0"
        onkeydown={(e) => e.key === "Enter" && store.setActive(p.id)}
      >
        <span class="dot" class:busy={st.busy}></span>
        <div class="meta">
          <div class="name">{p.name}</div>
          <div class="path">{p.path}</div>
        </div>
        <button class="del" onclick={(e) => removeProject(e, p.id)} title="Supprimer">×</button>
      </li>
    {/each}
    {#if store.projects.length === 0}
      <li class="empty">Aucun projet. Clique sur + pour ajouter un dossier.</li>
    {/if}
  </ul>
</aside>

<style>
  .sidebar {
    width: 280px;
    min-width: 280px;
    background: #141418;
    border-right: 1px solid #26262d;
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    border-bottom: 1px solid #26262d;
  }
  h1 {
    font-size: 13px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: #8a8a96;
    margin: 0;
  }
  .add-btn {
    background: #2a2a33;
    border: none;
    color: #e8e8ee;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 18px;
    line-height: 1;
  }
  .add-btn:hover {
    background: #3a3a45;
  }
  ul {
    list-style: none;
    padding: 6px;
    margin: 0;
    overflow-y: auto;
    flex: 1;
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: 8px;
    cursor: pointer;
    color: #c8c8d2;
    position: relative;
  }
  li:hover {
    background: #1e1e25;
  }
  li.active {
    background: #2b2b36;
  }
  li.empty {
    color: #6a6a75;
    font-size: 13px;
    cursor: default;
    padding: 16px;
  }
  li.empty:hover {
    background: transparent;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #444;
    flex-shrink: 0;
  }
  .dot.busy {
    background: #d97706;
    box-shadow: 0 0 0 3px rgba(217, 119, 6, 0.2);
    animation: pulse 1.2s infinite;
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.4;
    }
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .name {
    font-size: 13px;
    font-weight: 500;
    color: #e8e8ee;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .path {
    font-size: 11px;
    color: #6a6a75;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .del {
    background: transparent;
    border: none;
    color: #6a6a75;
    font-size: 18px;
    cursor: pointer;
    opacity: 0;
    padding: 0 4px;
  }
  li:hover .del {
    opacity: 1;
  }
  .del:hover {
    color: #ef4444;
  }
</style>
