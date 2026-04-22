<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    open = $bindable(),
    title,
    children,
  }: {
    open: boolean;
    title: string;
    children: Snippet;
  } = $props();

  function close() {
    open = false;
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if open}
  <div
    class="fixed inset-0 z-50 bg-black/50 flex items-center justify-center p-4"
    onclick={close}
    onkeydown={() => {}}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div
      class="bg-bg-1 border border-line rounded-xl shadow-2xl w-full max-w-3xl max-h-[80vh] flex flex-col overflow-hidden"
      onclick={(e) => e.stopPropagation()}
      onkeydown={() => {}}
      role="document"
    >
      <header
        class="shrink-0 flex items-center justify-between px-4 py-3 border-b border-line bg-bg-2"
      >
        <h2 class="text-text-0 text-sm font-semibold m-0">{title}</h2>
        <button
          type="button"
          onclick={close}
          aria-label="Close"
          class="bg-transparent border-none text-text-2 hover:text-text-0 cursor-pointer w-7 h-7 inline-flex items-center justify-center text-lg rounded-md hover:bg-bg-1"
        >
          ×
        </button>
      </header>
      <div class="flex-1 overflow-y-auto">
        {@render children()}
      </div>
    </div>
  </div>
{/if}
