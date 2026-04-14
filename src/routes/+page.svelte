<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "$lib/Sidebar.svelte";
  import Chat from "$lib/Chat.svelte";
  import RightPanel from "$lib/RightPanel.svelte";
  import { store } from "$lib/store.svelte";

  let rightCollapsed = $state(false);

  onMount(async () => {
    await store.initListener();
    await store.refreshProjects();
  });
</script>

<div class="app">
  <Sidebar />
  <Chat />
  <RightPanel bind:collapsed={rightCollapsed} />
</div>

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    height: 100%;
    background: #0e0e12;
    color: #e8e8ee;
    font-family:
      -apple-system, BlinkMacSystemFont, "Segoe UI", Inter, Roboto, sans-serif;
    font-size: 14px;
  }
  :global(*) {
    box-sizing: border-box;
  }
  .app {
    display: flex;
    height: 100vh;
    width: 100vw;
  }
</style>
