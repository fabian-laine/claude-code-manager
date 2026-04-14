<script lang="ts">
  import { store } from "./store.svelte";
  import Message from "./Message.svelte";

  let input = $state("");
  let scrollEl: HTMLDivElement | undefined = $state();

  const active = $derived(
    store.activeId ? store.projects.find((p) => p.id === store.activeId) : null,
  );
  const st = $derived(store.activeState);

  async function send() {
    const text = input.trim();
    if (!text || !store.activeId || st?.busy) return;
    input = "";
    await store.sendMessage(store.activeId, text);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  function resume() {
    if (!store.activeId) return;
    const text = input;
    input = "";
    store.resumeWithGuidance(store.activeId, text);
  }

  function pause() {
    if (!store.activeId) return;
    store.pauseMessage(store.activeId);
  }

  function abort() {
    if (!store.activeId) return;
    store.cancelMessage(store.activeId);
  }

  $effect(() => {
    const _ = st?.messages.length;
    if (scrollEl) {
      queueMicrotask(() => {
        if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
      });
    }
  });
</script>

<section class="flex-1 flex flex-col bg-bg-0 min-w-0 min-h-0">
  {#if !active}
    <div class="flex-1 flex items-center justify-center text-text-3 text-[13px]">
      <p>Select or add a project to get started.</p>
    </div>
  {:else}
    <div class="flex-1 overflow-y-auto px-6 py-4" bind:this={scrollEl}>
      {#if st && st.messages.length === 0}
        <div class="text-text-3 text-[13px] text-center py-10">
          Send your first message to Claude.
        </div>
      {/if}
      {#each st?.messages ?? [] as m (m.id)}
        <Message msg={m} />
      {/each}
    </div>

    <div class="shrink-0 p-4 border-t border-line bg-bg-1">
      {#if st?.paused}
        <div
          class="relative bg-bg-2 border border-line-2 rounded-xl focus-within:border-info transition-colors"
        >
          <textarea
            placeholder="Add extra instructions for Claude (optional), then Resume…"
            bind:value={input}
            rows="2"
            class="w-full bg-transparent border-none text-text-0 text-sm leading-relaxed resize-none outline-none pl-3.5 pr-14 py-3 min-h-[46px] max-h-[200px] font-sans"
          ></textarea>
          <button
            onclick={resume}
            title={input.trim() ? "Resume with these instructions" : "Resume"}
            aria-label="Resume"
            class="absolute right-2 bottom-2 w-9 h-9 rounded-[10px] border-none cursor-pointer inline-flex items-center justify-center bg-ok hover:brightness-110 text-bg-1"
          >
            <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
              <path d="M8 5v14l11-7z" />
            </svg>
          </button>
        </div>
        <button
          onclick={abort}
          class="block mx-auto mt-2 bg-transparent border-none text-text-2 hover:text-[#f87171] hover:underline text-xs cursor-pointer py-1 px-2"
        >
          Abort request
        </button>
      {:else}
        <div
          class="relative bg-bg-2 border {st?.busy
            ? 'border-[#3a2e20]'
            : 'border-line-2 focus-within:border-info'} rounded-xl transition-colors"
        >
          <textarea
            placeholder={st?.busy
              ? "Claude is working…"
              : "Message Claude… (Enter to send, Shift+Enter for new line)"}
            bind:value={input}
            onkeydown={onKey}
            disabled={st?.busy}
            rows="2"
            class="w-full bg-transparent border-none text-text-0 disabled:text-text-2 text-sm leading-relaxed resize-none outline-none pl-3.5 pr-14 py-3 min-h-[46px] max-h-[200px] font-sans"
          ></textarea>
          {#if st?.busy}
            <button
              onclick={pause}
              title="Pause Claude"
              aria-label="Pause"
              class="absolute right-2 bottom-2 w-9 h-9 rounded-[10px] border-none cursor-pointer inline-flex items-center justify-center bg-warn hover:brightness-110 text-bg-1"
            >
              <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                <rect x="6" y="5" width="4" height="14" rx="1" />
                <rect x="14" y="5" width="4" height="14" rx="1" />
              </svg>
            </button>
          {:else}
            <button
              onclick={send}
              disabled={!input.trim()}
              title="Send"
              aria-label="Send"
              class="absolute right-2 bottom-2 w-9 h-9 rounded-[10px] border-none cursor-pointer inline-flex items-center justify-center bg-accent hover:bg-accent-2 text-bg-1 disabled:bg-line-2 disabled:text-text-3 disabled:cursor-not-allowed"
            >
              <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
                <path d="M2 21l21-9L2 3v7l15 2-15 2z" />
              </svg>
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</section>
