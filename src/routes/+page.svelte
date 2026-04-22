<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "$lib/Sidebar.svelte";
  import Chat from "$lib/Chat.svelte";
  import RightPanel from "$lib/RightPanel.svelte";
  import Settings from "$lib/Settings.svelte";
  import WebLogin from "$lib/WebLogin.svelte";
  import { store } from "$lib/store.svelte";
  import { isTauri, getWebToken } from "$lib/api";

  let settingsOpen = $state(false);
  let webAuthed = $state(isTauri || !!getWebToken());
  let sidebarOpen = $state(true);
  let rightOpen = $state(true);
  let isMobile = $state(false);

  function updateMobile() {
    if (typeof window === "undefined") return;
    const wasMobile = isMobile;
    isMobile = window.innerWidth < 900;
    if (isMobile && !wasMobile) {
      sidebarOpen = false;
      rightOpen = false;
    }
  }

  async function init() {
    await store.initListener();
    await store.refreshProjects();
  }

  onMount(() => {
    if (webAuthed) init();
    updateMobile();
    window.addEventListener("resize", updateMobile);
    return () => window.removeEventListener("resize", updateMobile);
  });

  function onLogin() {
    webAuthed = true;
    init();
  }

  function toggleLeft() {
    sidebarOpen = !sidebarOpen;
    if (isMobile && sidebarOpen) rightOpen = false;
  }

  function toggleRight() {
    rightOpen = !rightOpen;
    if (isMobile && rightOpen) sidebarOpen = false;
  }

  function onProjectPicked() {
    if (isMobile) sidebarOpen = false;
  }

  const activeProject = $derived(
    store.activeId ? store.projects.find((p) => p.id === store.activeId) : null,
  );
  const activeState = $derived(store.activeState);
</script>

{#if !webAuthed}
  <WebLogin {onLogin} />
{:else}
  <div class="flex flex-col h-screen w-screen overflow-hidden">
    <!-- Top bar -->
    <header
      class="shrink-0 h-[52px] flex items-center gap-2.5 px-3 bg-bg-1 border-b border-line z-10"
    >
      <button
        class="shrink-0 w-[38px] h-[38px] rounded-lg border border-line text-text-1 hover:bg-bg-2 hover:text-text-0 inline-flex items-center justify-center text-lg cursor-pointer"
        onclick={toggleLeft}
        title="Toggle projects"
        aria-label="Toggle projects"
      >
        ☰
      </button>
      <div
        class="flex-1 min-w-0 flex items-center justify-center gap-2 text-text-0 font-semibold overflow-hidden"
      >
        {#if activeState?.busy}
          <span
            class="shrink-0 w-2 h-2 rounded-full bg-warn animate-pulse shadow-[0_0_0_3px_rgba(217,119,6,0.2)]"
          ></span>
        {/if}
        <span class="truncate">
          {activeProject?.name ?? "Claude Code Manager"}
        </span>
      </div>
      <button
        class="shrink-0 w-[38px] h-[38px] rounded-lg border border-line text-text-1 hover:bg-bg-2 hover:text-text-0 inline-flex items-center justify-center text-lg cursor-pointer"
        onclick={toggleRight}
        title="Toggle info panel"
        aria-label="Toggle info panel"
      >
        ⓘ
      </button>
    </header>

    <!-- Body -->
    <main class="flex-1 flex min-h-0 overflow-hidden">
      {#if sidebarOpen}
        <div
          class="shrink-0 h-full bg-bg-1 border-r border-line overflow-hidden w-full md:w-[280px]"
        >
          <Sidebar
            onOpenSettings={() => (settingsOpen = true)}
            onProjectSelect={onProjectPicked}
          />
        </div>
      {/if}
      {#if !(isMobile && (sidebarOpen || rightOpen))}
        <div class="flex-1 min-w-0 flex flex-col">
          <Chat
            onOpenSettings={() => (settingsOpen = true)}
            onToggleRightPanel={toggleRight}
          />
        </div>
      {/if}
      {#if rightOpen}
        <div
          class="shrink-0 h-full bg-bg-1 border-l border-line overflow-hidden w-full md:w-[300px]"
        >
          <RightPanel />
        </div>
      {/if}
    </main>
  </div>
{/if}

<Settings bind:open={settingsOpen} />
