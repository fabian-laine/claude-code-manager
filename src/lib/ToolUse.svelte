<script lang="ts">
  let { name, input }: { name: string; input: any } = $props();

  let expanded = $state(false);

  function summary(): string {
    const i = input ?? {};
    switch (name) {
      case "Read":
        return i.file_path ?? "";
      case "Write":
        return i.file_path ?? "";
      case "Edit":
        return i.file_path ?? "";
      case "Bash":
        return (i.command ?? "").split("\n")[0].slice(0, 200);
      case "Grep":
        return `${i.pattern ?? ""}${i.path ? "  in " + i.path : ""}`;
      case "Glob":
        return i.pattern ?? "";
      case "TodoWrite":
        return `${(i.todos ?? []).length} items`;
      case "WebFetch":
        return i.url ?? "";
      case "WebSearch":
        return i.query ?? "";
      case "Task":
        return i.description ?? i.subagent_type ?? "";
      default:
        try {
          return JSON.stringify(i).slice(0, 120);
        } catch {
          return "";
        }
    }
  }

  function icon(): string {
    switch (name) {
      case "Read": return "📖";
      case "Write": return "📝";
      case "Edit": return "✎";
      case "Bash": return "$";
      case "Grep": return "🔍";
      case "Glob": return "*";
      case "TodoWrite": return "☐";
      case "WebFetch":
      case "WebSearch": return "🌐";
      case "Task": return "🤖";
      default: return "⚙";
    }
  }
</script>

<div class="tool">
  <button class="head" onclick={() => (expanded = !expanded)}>
    <span class="ico">{icon()}</span>
    <span class="nm">{name}</span>
    <span class="sum">{summary()}</span>
    <span class="chev">{expanded ? "▾" : "▸"}</span>
  </button>
  {#if expanded}
    <div class="body">
      {#if name === "Bash" && input?.command}
        <pre class="code bash">$ {input.command}</pre>
        {#if input.description}
          <div class="desc">{input.description}</div>
        {/if}
      {:else if name === "Edit" && input?.old_string !== undefined}
        <div class="diff">
          <div class="diff-path">{input.file_path ?? ""}</div>
          <pre class="del">{input.old_string || "(empty)"}</pre>
          <pre class="add">{input.new_string || "(empty)"}</pre>
        </div>
      {:else if name === "Write" && input?.content}
        <pre class="code">{input.content}</pre>
      {:else if name === "TodoWrite" && Array.isArray(input?.todos)}
        <ul class="todos">
          {#each input.todos as t}
            <li class={t.status}>
              <span class="box">{t.status === "completed" ? "☑" : t.status === "in_progress" ? "◐" : "☐"}</span>
              <span>{t.content}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <pre class="code">{JSON.stringify(input, null, 2)}</pre>
      {/if}
    </div>
  {/if}
</div>

<style>
  .tool {
    margin: 6px 0;
    background: #16161c;
    border: 1px solid #26262d;
    border-radius: 8px;
    overflow: hidden;
  }
  .head {
    display: flex;
    width: 100%;
    gap: 10px;
    align-items: center;
    padding: 8px 12px;
    background: transparent;
    border: none;
    color: #c8c8d2;
    cursor: pointer;
    text-align: left;
    font-family: inherit;
    font-size: 13px;
  }
  .head:hover {
    background: #1b1b23;
  }
  .ico {
    width: 18px;
    text-align: center;
    color: #7aa870;
  }
  .nm {
    font-weight: 600;
    color: #7aa870;
    min-width: 60px;
  }
  .sum {
    flex: 1;
    color: #9a9aa5;
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chev {
    color: #555;
    font-size: 10px;
  }
  .body {
    padding: 10px 12px;
    border-top: 1px solid #26262d;
    background: #0e0e12;
  }
  .code {
    margin: 0;
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 12px;
    color: #c8c8d2;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 400px;
    overflow-y: auto;
  }
  .code.bash {
    color: #e8e8ee;
  }
  .desc {
    color: #6a6a75;
    font-size: 11px;
    margin-top: 6px;
    font-style: italic;
  }
  .diff-path {
    font-size: 11px;
    color: #6a6a75;
    margin-bottom: 6px;
    font-family: "JetBrains Mono", ui-monospace, monospace;
  }
  .diff pre {
    margin: 0 0 4px 0;
    padding: 6px 10px;
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 12px;
    white-space: pre-wrap;
    border-radius: 4px;
    max-height: 250px;
    overflow-y: auto;
  }
  .diff pre.del {
    background: #2a1414;
    color: #f87171;
    border-left: 3px solid #5a2a2a;
  }
  .diff pre.add {
    background: #0f2416;
    color: #86efac;
    border-left: 3px solid #2a5a3a;
  }
  .todos {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .todos li {
    display: flex;
    gap: 8px;
    padding: 3px 0;
    color: #c8c8d2;
    font-size: 13px;
  }
  .todos li.completed {
    color: #6a6a75;
    text-decoration: line-through;
  }
  .todos li.in_progress {
    color: #c9a96e;
  }
  .box {
    width: 16px;
    color: #7aa870;
  }
</style>
