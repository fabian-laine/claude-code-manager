<script lang="ts">
  import type { SlashCommand } from "./slashCommands";

  let {
    commands,
    selectedIdx,
    onPick,
  }: {
    commands: SlashCommand[];
    selectedIdx: number;
    onPick: (cmd: SlashCommand) => void;
  } = $props();
</script>

{#if commands.length > 0}
  <div
    class="absolute bottom-full left-0 right-0 mb-2 bg-bg-2 border border-line rounded-xl shadow-xl overflow-hidden max-h-64 overflow-y-auto z-20"
  >
    <div
      class="px-3 py-1.5 text-[10px] uppercase tracking-wider text-text-3 border-b border-line"
    >
      Slash commands
    </div>
    {#each commands as cmd, i (cmd.name)}
      <button
        type="button"
        class="w-full text-left px-3.5 py-2 flex items-center gap-3 bg-transparent border-none cursor-pointer {i ===
        selectedIdx
          ? 'bg-bg-1'
          : 'hover:bg-bg-1'}"
        onmousedown={(e) => {
          e.preventDefault();
          onPick(cmd);
        }}
      >
        <span class="text-accent font-mono text-[13px] shrink-0 min-w-24"
          >/{cmd.name}</span
        >
        <span class="text-text-2 text-[12px] flex-1">{cmd.description}</span>
      </button>
    {/each}
  </div>
{/if}
