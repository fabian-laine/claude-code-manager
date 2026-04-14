<script lang="ts">
  import type { RenderedMessage } from "./types";
  import ToolUse from "./ToolUse.svelte";
  import Markdown from "./Markdown.svelte";
  let { msg }: { msg: RenderedMessage } = $props();

  let resultExpanded = $state(false);
</script>

{#if msg.type === "user"}
  <div class="msg user">
    <div class="label">You</div>
    <div class="body">{msg.text}</div>
  </div>
{:else if msg.type === "assistant_text"}
  <div class="msg assistant">
    <div class="label">Claude</div>
    <div class="body"><Markdown source={msg.text} /></div>
  </div>
{:else if msg.type === "thinking"}
  <div class="msg thinking">
    <div class="label">thinking</div>
    <div class="body">{msg.text}</div>
  </div>
{:else if msg.type === "tool_use"}
  <ToolUse name={msg.name} input={msg.input} />
{:else if msg.type === "tool_result"}
  {@const lines = msg.content.split("\n")}
  {@const preview = lines.slice(0, 6).join("\n")}
  {@const hasMore = lines.length > 6 || msg.content.length > 500}
  <div class="msg tool-result" class:err={msg.is_error}>
    <button class="result-head" onclick={() => (resultExpanded = !resultExpanded)}>
      <span class="result-label">{msg.is_error ? "error" : "result"}</span>
      <span class="result-meta">{lines.length} ligne{lines.length > 1 ? "s" : ""}</span>
      {#if hasMore}
        <span class="chev">{resultExpanded ? "▾" : "▸"}</span>
      {/if}
    </button>
    <pre>{resultExpanded || !hasMore ? msg.content : preview + "\n…"}</pre>
  </div>
{:else if msg.type === "error"}
  <div class="msg error">
    <div class="label">error</div>
    <div class="body">{msg.text}</div>
  </div>
{/if}

<style>
  .msg {
    margin: 14px 0;
    padding: 14px 18px;
    border-radius: 10px;
    font-size: 14px;
    line-height: 1.55;
  }
  .label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #7a7a85;
    margin-bottom: 10px;
  }
  .body {
    white-space: pre-wrap;
    word-wrap: break-word;
    color: #e8e8ee;
  }
  .user {
    background: #1e2a3a;
    border: 1px solid #2a3f5a;
  }
  .user .label {
    color: #6aa0d0;
  }
  .assistant {
    background: #1a1a22;
    border: 1px solid #2a2a33;
  }
  .assistant .label {
    color: #c9a96e;
  }
  .thinking {
    background: transparent;
    border-left: 2px solid #4a4a5a;
    padding: 4px 12px;
    margin-left: 6px;
  }
  .thinking .label {
    color: #6a6a75;
  }
  .thinking .body {
    color: #8a8a95;
    font-style: italic;
    font-size: 13px;
  }
  .tool-result {
    background: #10101a;
    border: 1px solid #26262d;
    padding: 0;
    margin: 2px 0 10px 18px;
    border-radius: 6px;
    font-size: 12px;
  }
  .result-head {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 6px 12px;
    background: transparent;
    border: none;
    color: #7a7a85;
    cursor: pointer;
    font-family: inherit;
    width: 100%;
    text-align: left;
  }
  .result-head:hover {
    background: #16161c;
  }
  .result-label {
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-size: 10px;
    color: #6a6a75;
  }
  .result-meta {
    flex: 1;
    font-size: 11px;
    color: #6a6a75;
  }
  .chev {
    font-size: 10px;
    color: #555;
  }
  .tool-result pre {
    margin: 0;
    padding: 0 12px 10px 12px;
    font-size: 12px;
    color: #9a9aa5;
    white-space: pre-wrap;
    max-height: 400px;
    overflow-y: auto;
  }
  .tool-result.err {
    border-color: #5a2a2a;
  }
  .tool-result.err pre {
    color: #ef8888;
  }
  .error {
    background: #2a1a1a;
    border: 1px solid #5a2a2a;
  }
  .error .label {
    color: #ef8888;
  }
  pre {
    font-family: "JetBrains Mono", "Fira Code", ui-monospace, monospace;
  }
</style>
