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

  $effect(() => {
    // auto-scroll when messages grow
    const _ = st?.messages.length;
    if (scrollEl) {
      queueMicrotask(() => {
        if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
      });
    }
  });
</script>

<section class="chat">
  {#if !active}
    <div class="empty-state">
      <p>Sélectionne ou ajoute un projet pour commencer.</p>
    </div>
  {:else}
    <header>
      <div class="title">{active.name}</div>
      <div class="subtitle">{active.path}</div>
      {#if st?.busy}
        <div class="status">Claude travaille…</div>
      {/if}
    </header>
    <div class="scroll" bind:this={scrollEl}>
      {#if st && st.messages.length === 0}
        <div class="empty-state">Envoie ton premier message à Claude.</div>
      {/if}
      {#each st?.messages ?? [] as m (m.id)}
        <Message msg={m} />
      {/each}
    </div>
    <footer>
      {#if st?.paused}
        <textarea
          placeholder="Ajoute des consignes pour Claude (optionnel), puis Reprendre…"
          bind:value={input}
          rows="3"
        ></textarea>
        <div class="col">
          <button
            class="resume"
            onclick={() => {
              if (!store.activeId) return;
              const text = input;
              input = "";
              store.resumeWithGuidance(store.activeId, text);
            }}
            title={input.trim() ? "Reprendre avec ces consignes" : "Reprendre"}
          >
            ▶ {input.trim() ? "Reprendre avec consignes" : "Reprendre"}
          </button>
          <button
            class="ghost"
            onclick={() => store.activeId && store.cancelMessage(store.activeId)}
            title="Abandonner cette requête"
          >
            Abandonner
          </button>
        </div>
      {:else if st?.busy}
        <div class="running">
          <span>Claude travaille…</span>
        </div>
        <button
          class="pause"
          onclick={() => store.activeId && store.pauseMessage(store.activeId)}
          title="Mettre Claude en pause"
        >
          ⏸ Pause
        </button>
      {:else}
        <textarea
          placeholder="Message à Claude… (Entrée pour envoyer, Shift+Entrée pour nouvelle ligne)"
          bind:value={input}
          onkeydown={onKey}
          rows="3"
        ></textarea>
        <button onclick={send} disabled={!input.trim()}>Envoyer</button>
      {/if}
    </footer>
  {/if}
</section>

<style>
  .chat {
    flex: 1;
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: #0e0e12;
    min-width: 0;
  }
  header {
    padding: 14px 20px;
    border-bottom: 1px solid #26262d;
    background: #14141a;
  }
  .title {
    font-size: 14px;
    font-weight: 600;
    color: #e8e8ee;
  }
  .subtitle {
    font-size: 11px;
    color: #6a6a75;
    margin-top: 2px;
  }
  .status {
    font-size: 11px;
    color: #d97706;
    margin-top: 4px;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 16px 24px;
  }
  .empty-state {
    color: #6a6a75;
    text-align: center;
    padding: 40px 20px;
    font-size: 13px;
  }
  footer {
    padding: 14px 20px;
    border-top: 1px solid #26262d;
    background: #14141a;
    display: flex;
    gap: 10px;
    align-items: flex-end;
  }
  textarea {
    flex: 1;
    background: #1a1a22;
    border: 1px solid #2a2a33;
    border-radius: 8px;
    padding: 10px 12px;
    color: #e8e8ee;
    font-family: inherit;
    font-size: 14px;
    resize: none;
    outline: none;
  }
  textarea:focus {
    border-color: #4a6a9a;
  }
  textarea:disabled {
    opacity: 0.5;
  }
  button {
    background: #c9a96e;
    color: #14141a;
    border: none;
    border-radius: 8px;
    padding: 10px 18px;
    font-weight: 600;
    cursor: pointer;
    font-size: 13px;
  }
  button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  button:hover:not(:disabled) {
    background: #d9b97e;
  }
  button.pause {
    background: #d97706;
    color: #14141a;
  }
  button.pause:hover {
    background: #eb8a14;
  }
  button.resume {
    background: #7aa870;
    color: #14141a;
  }
  button.resume:hover {
    background: #8cbd80;
  }
  button.ghost {
    background: transparent;
    color: #9a9aa5;
    border: 1px solid #3a3a45;
    font-weight: 500;
  }
  button.ghost:hover {
    background: #1e1e25;
    color: #e8e8ee;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .running {
    flex: 1;
    display: flex;
    align-items: center;
    padding: 0 14px;
    color: #d97706;
    font-size: 13px;
    background: #1a1a22;
    border: 1px solid #2a2a33;
    border-radius: 8px;
    height: 64px;
  }
</style>
