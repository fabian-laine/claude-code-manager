<script lang="ts">
  import { store } from "./store.svelte";
  import { isTauri } from "./api";

  let {
    onOpenSettings,
    onProjectSelect,
  }: {
    onOpenSettings: () => void;
    onProjectSelect?: () => void;
  } = $props();

  function pick(id: string) {
    store.setActive(id);
    onProjectSelect?.();
  }

  const sortedProjects = $derived(
    [...store.projects].sort((a, b) =>
      a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
    ),
  );

  async function addProject() {
    if (isTauri) {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({ directory: true, multiple: false });
      if (!picked || typeof picked !== "string") return;
      const name = picked.split("/").filter(Boolean).pop() ?? picked;
      await store.addProject(name, picked);
    } else {
      const path = prompt("Absolute path of the project on the PC:");
      if (!path) return;
      const name = path.split("/").filter(Boolean).pop() ?? path;
      await store.addProject(name, path);
    }
  }

  async function removeProject(e: MouseEvent, id: string) {
    e.stopPropagation();
    if (!confirm("Delete this project?")) return;
    await store.deleteProject(id);
  }
</script>

<aside class="flex flex-col h-full w-full bg-bg-1">
  <header class="flex items-center justify-between px-4 py-3.5 border-b border-line">
    <h1 class="text-xs uppercase tracking-widest text-text-2 font-semibold m-0">
      Projects
    </h1>
    <button
      onclick={addProject}
      title="Add a project"
      aria-label="Add a project"
      class="w-6.5 h-6.5 rounded-md bg-line-2 hover:bg-[#3a3a45] text-text-0 text-lg leading-none cursor-pointer border-none"
    >
      +
    </button>
  </header>

  <ul class="list-none m-0 p-1.5 flex-1 min-h-0 overflow-y-auto">
    {#each sortedProjects as p (p.id)}
      {@const st = store.stateFor(p.id)}
      <li
        class="group flex items-center gap-2.5 px-3 py-2.5 rounded-lg cursor-pointer text-text-1 relative overflow-hidden hover:bg-bg-2 {store.activeId ===
        p.id
          ? 'bg-[#2b2b36]'
          : ''}"
        onclick={() => pick(p.id)}
        role="button"
        tabindex="0"
        onkeydown={(e) => e.key === "Enter" && pick(p.id)}
      >
        <span
          class="shrink-0 w-2 h-2 rounded-full {st.busy
            ? 'bg-warn shadow-[0_0_0_3px_rgba(217,119,6,0.2)] animate-pulse'
            : st.hasUnseenFinish
              ? 'bg-ok shadow-[0_0_0_3px_rgba(122,168,112,0.2)]'
              : 'bg-[#444]'}"
        ></span>
        <div class="flex-1 min-w-0 overflow-hidden">
          <div class="text-[13px] font-medium text-text-0 truncate">
            {p.name}
          </div>
          <div class="text-[11px] text-text-3 truncate">{p.path}</div>
        </div>
        <button
          onclick={(e) => removeProject(e, p.id)}
          title="Delete"
          aria-label="Delete"
          class="opacity-0 group-hover:opacity-100 bg-transparent border-none text-text-3 hover:text-[#ef4444] text-lg cursor-pointer px-1"
        >
          ×
        </button>
      </li>
    {/each}
    {#if store.projects.length === 0}
      <li class="text-text-3 text-[13px] p-4 text-center">
        No projects yet. Click + to add a folder.
      </li>
    {/if}
  </ul>

  {#if isTauri}
    <footer class="border-t border-line p-2">
      <button
        onclick={onOpenSettings}
        title="Settings"
        class="w-full bg-transparent border border-line text-text-2 hover:bg-bg-2 hover:text-text-0 px-3 py-2 rounded-md cursor-pointer text-[13px] flex items-center gap-2 justify-center"
      >
        <span>⚙</span> Settings
      </button>
    </footer>
  {/if}
</aside>
