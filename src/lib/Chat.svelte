<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { store } from "./store.svelte";
  import {
    isTauri,
    type SessionStats,
    type GlobalStats,
    type ProjectFlags,
  } from "./api";
  import Message from "./Message.svelte";
  import SlashPalette from "./SlashPalette.svelte";
  import SimpleModal from "./SimpleModal.svelte";
  import { matchCommands, slashCommands, type SlashCommand } from "./slashCommands";
  import { MicRecorder } from "./audio";

  let {
    onOpenSettings,
    onToggleRightPanel,
  }: {
    onOpenSettings?: () => void;
    onToggleRightPanel?: () => void;
  } = $props();

  type Attachment = { name: string; refPath: string };

  let input = $state("");
  let scrollEl: HTMLDivElement | undefined = $state();
  let textareaEl: HTMLTextAreaElement | undefined = $state();
  let lastMessageCount = 0;
  let pinnedToBottom = true;
  let autoLoadGuard = false;
  let slashSelectedIdx = $state(0);
  let attachments = $state<Attachment[]>([]);
  let uploading = $state(false);
  let dragging = $state(false);
  let uploadError = $state<string | null>(null);

  let helpOpen = $state(false);
  let diffOpen = $state(false);
  let diffText = $state("");
  let diffLoading = $state(false);
  let diffError = $state<string | null>(null);

  type EditorTab = {
    label: string;
    path: string;
    readonly?: boolean;
    content: string;
    loaded: boolean;
    dirty: boolean;
    error: string | null;
  };
  let editorOpen = $state(false);
  let editorTitle = $state("");
  let editorTabs = $state<EditorTab[]>([]);
  let editorActiveIdx = $state(0);
  let editorSaveMsg = $state<string | null>(null);

  type BrowserEntry = { name: string; path: string };
  type BrowserDir = {
    label: string;
    path: string;
    entries: BrowserEntry[];
    error: string | null;
  };
  let browserOpen = $state(false);
  let browserTitle = $state("");
  let browserDirs = $state<BrowserDir[]>([]);
  let browserSelectedPath = $state<string | null>(null);
  let browserSelectedContent = $state("");
  let browserSelectedError = $state<string | null>(null);

  let statsModalOpen = $state(false);
  let statsModalTitle = $state("");
  let statsSession = $state<SessionStats | null>(null);
  let statsGlobal = $state<GlobalStats | null>(null);
  let statsMode = $state<"cost" | "context" | "stats">("cost");
  let statsError = $state<string | null>(null);

  let flagsModalOpen = $state(false);
  let flagsModalTitle = $state("");
  let flagsModalKind = $state<"model" | "effort" | "add-dir">("model");
  let flagsCurrent = $state<ProjectFlags | null>(null);
  let flagsNewDir = $state("");

  const MODEL_OPTIONS: { value: string; label: string }[] = [
    { value: "", label: "Default (let CLI decide)" },
    { value: "haiku", label: "Haiku (fast, cheap)" },
    { value: "sonnet", label: "Sonnet (balanced)" },
    { value: "opus", label: "Opus (most capable)" },
  ];
  const EFFORT_OPTIONS: { value: string; label: string }[] = [
    { value: "", label: "Default" },
    { value: "low", label: "Low" },
    { value: "medium", label: "Medium" },
    { value: "high", label: "High" },
    { value: "xhigh", label: "Extra High" },
  ];

  function formatTokens(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)} M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)} k`;
    return n.toLocaleString();
  }

  // Rough US$ per 1M tokens, indicative only. Updated 2026-04.
  function pricePer1M(model: string | null, kind: "in" | "out" | "cache_w" | "cache_r"): number {
    const m = (model ?? "").toLowerCase();
    if (m.includes("opus")) {
      return kind === "in" ? 15 : kind === "out" ? 75 : kind === "cache_w" ? 18.75 : 1.5;
    }
    if (m.includes("haiku")) {
      return kind === "in" ? 1 : kind === "out" ? 5 : kind === "cache_w" ? 1.25 : 0.1;
    }
    // default Sonnet
    return kind === "in" ? 3 : kind === "out" ? 15 : kind === "cache_w" ? 3.75 : 0.3;
  }

  function estimateCost(s: SessionStats): number {
    return (
      (s.input_tokens * pricePer1M(s.model, "in")) / 1_000_000 +
      (s.output_tokens * pricePer1M(s.model, "out")) / 1_000_000 +
      (s.cache_creation_tokens * pricePer1M(s.model, "cache_w")) / 1_000_000 +
      (s.cache_read_tokens * pricePer1M(s.model, "cache_r")) / 1_000_000
    );
  }

  // Rough context window sizes per model family (tokens).
  function contextWindow(model: string | null): number {
    const m = (model ?? "").toLowerCase();
    if (m.includes("1m")) return 1_000_000;
    return 200_000;
  }

  async function loadSessionStats() {
    statsError = null;
    statsSession = null;
    try {
      const api = await store.ensureApi();
      if (!api.sessionStats || !store.activeId)
        throw new Error("not available");
      statsSession = await api.sessionStats(store.activeId);
    } catch (e) {
      statsError = String(e);
    }
  }

  async function loadGlobalStats() {
    statsError = null;
    statsGlobal = null;
    try {
      const api = await store.ensureApi();
      if (!api.globalStats) throw new Error("not available");
      statsGlobal = await api.globalStats();
    } catch (e) {
      statsError = String(e);
    }
  }

  function openCost() {
    statsMode = "cost";
    statsModalTitle = "Session cost";
    statsModalOpen = true;
    loadSessionStats();
  }
  function openContextStats() {
    statsMode = "context";
    statsModalTitle = "Context usage";
    statsModalOpen = true;
    loadSessionStats();
  }
  function openGlobalStats() {
    statsMode = "stats";
    statsModalTitle = "Usage across all sessions";
    statsModalOpen = true;
    loadGlobalStats();
  }

  async function loadFlags() {
    if (!store.activeId) return;
    const api = await store.ensureApi();
    if (!api.getProjectFlags) return;
    flagsCurrent = await api.getProjectFlags(store.activeId);
  }

  async function openModelPicker() {
    flagsModalKind = "model";
    flagsModalTitle = "Model for this project";
    flagsModalOpen = true;
    await loadFlags();
  }
  async function openEffortPicker() {
    flagsModalKind = "effort";
    flagsModalTitle = "Effort level for this project";
    flagsModalOpen = true;
    await loadFlags();
  }
  async function openAddDir() {
    flagsModalKind = "add-dir";
    flagsModalTitle = "Additional working directories";
    flagsModalOpen = true;
    flagsNewDir = "";
    await loadFlags();
  }

  async function pickFlag(value: string) {
    if (!store.activeId) return;
    const api = await store.ensureApi();
    if (!api.setProjectFlag) return;
    await api.setProjectFlag(store.activeId, flagsModalKind, value);
    await loadFlags();
  }

  async function addDir() {
    if (!store.activeId || !flagsNewDir.trim()) return;
    const api = await store.ensureApi();
    if (!api.addProjectDir) return;
    flagsCurrent = { ...flagsCurrent!, add_dirs: await api.addProjectDir(store.activeId, flagsNewDir.trim()) };
    flagsNewDir = "";
  }

  async function removeDir(p: string) {
    if (!store.activeId) return;
    const api = await store.ensureApi();
    if (!api.removeProjectDir) return;
    flagsCurrent = { ...flagsCurrent!, add_dirs: await api.removeProjectDir(store.activeId, p) };
  }

  let sttAvailable = $state(false);
  let sttModelReady = $state(false);
  let recording = $state(false);
  let transcribing = $state(false);
  let sttError = $state<string | null>(null);
  let recorder: MicRecorder | null = null;

  onMount(async () => {
    try {
      const api = await store.ensureApi();
      const status = await api.sttStatus();
      sttAvailable = status.available;
      sttModelReady = status.model_ready;
    } catch {
      /* silently disable */
    }
  });

  async function toggleRecording() {
    sttError = null;
    if (!sttAvailable) {
      sttError =
        "Speech-to-text isn't compiled in. Rebuild with --features stt (see README).";
      return;
    }
    if (recording) {
      await stopAndTranscribe();
      return;
    }
    if (!sttModelReady) {
      try {
        transcribing = true;
        const api = await store.ensureApi();
        await api.sttDownloadModel();
        sttModelReady = true;
      } catch (e) {
        sttError = `Model download failed: ${String(e)}`;
        transcribing = false;
        return;
      } finally {
        transcribing = false;
      }
    }
    try {
      recorder = new MicRecorder();
      await recorder.start();
      recording = true;
    } catch (e) {
      sttError = `Microphone access denied: ${String(e)}`;
      recorder = null;
    }
  }

  async function stopAndTranscribe() {
    if (!recorder) return;
    transcribing = true;
    recording = false;
    try {
      const wav = await recorder.stop();
      recorder = null;
      if (wav.length <= 44) {
        sttError = "No audio captured.";
        return;
      }
      const api = await store.ensureApi();
      // Pass the browser locale (e.g. "fr-FR" → "fr") so whisper doesn't
      // silently default to English when detection is uncertain.
      const lang =
        typeof navigator !== "undefined" && navigator.language
          ? navigator.language.split("-")[0]
          : undefined;
      const text = await api.sttTranscribe(wav, lang);
      if (text.trim()) {
        input = input ? `${input} ${text}` : text;
        queueMicrotask(() => textareaEl?.focus());
      }
    } catch (e) {
      sttError = `Transcription failed: ${String(e)}`;
    } finally {
      transcribing = false;
    }
  }

  const slashRaw = $derived.by(() => {
    const t = input;
    if (!t.startsWith("/")) return null;
    if (t.includes("\n")) return null;
    return t.slice(1);
  });
  const slashCmdName = $derived<string | null>(
    slashRaw === null ? null : slashRaw.split(/\s+/)[0] ?? "",
  );
  const slashArgs = $derived.by(() => {
    if (slashRaw === null) return "";
    const space = slashRaw.indexOf(" ");
    return space === -1 ? "" : slashRaw.slice(space + 1).trim();
  });
  const slashMatches = $derived<SlashCommand[]>(
    slashCmdName === null ? [] : matchCommands(slashCmdName),
  );
  const slashVisible = $derived(slashCmdName !== null && slashMatches.length > 0);

  $effect(() => {
    // Reset selection whenever the filtered list changes
    const _ = slashMatches.length;
    slashSelectedIdx = 0;
  });

  const active = $derived(
    store.activeId ? store.projects.find((p) => p.id === store.activeId) : null,
  );
  const st = $derived(store.activeState);

  async function tryLoadMoreHistory() {
    if (!store.activeId || !scrollEl || !st) return;
    if (!st.hasMoreHistory || st.loadingMore || autoLoadGuard) return;
    if (scrollEl.scrollTop > 80) return;
    autoLoadGuard = true;
    const prevHeight = scrollEl.scrollHeight;
    const prevTop = scrollEl.scrollTop;
    const added = await store.loadMoreHistory(store.activeId);
    await new Promise((r) => requestAnimationFrame(() => r(null)));
    if (scrollEl && added > 0) {
      const delta = scrollEl.scrollHeight - prevHeight;
      scrollEl.scrollTop = prevTop + delta;
    }
    autoLoadGuard = false;
  }

  function onScroll() {
    if (!scrollEl) return;
    const threshold = 40;
    pinnedToBottom =
      scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight <
      threshold;
    tryLoadMoreHistory();
  }

  function composeMessageText(text: string, atts: Attachment[]): string {
    if (atts.length === 0) return text;
    const refs = atts.map((a) => `@${a.refPath}`).join(" ");
    return text.length > 0 ? `${text}\n\n${refs}` : refs;
  }

  async function send() {
    const hasAttachments = attachments.length > 0;
    const text = input.trim();
    if ((!text && !hasAttachments) || !store.activeId || st?.busy) return;
    const finalText = composeMessageText(text, attachments);
    input = "";
    attachments = [];
    await store.sendMessage(store.activeId, finalText);
  }

  async function uploadFiles(files: FileList | File[]) {
    if (!store.activeId || files.length === 0) return;
    uploadError = null;
    uploading = true;
    try {
      const api = await store.ensureApi();
      const added: Attachment[] = [];
      for (const f of Array.from(files)) {
        const buf = new Uint8Array(await f.arrayBuffer());
        const refPath = await api.saveAttachment(store.activeId, f.name, buf);
        added.push({ name: f.name, refPath });
      }
      attachments = [...attachments, ...added];
    } catch (e) {
      uploadError = String(e);
    } finally {
      uploading = false;
    }
  }

  function onFilePick(e: Event) {
    const t = e.target as HTMLInputElement;
    if (t.files) uploadFiles(t.files);
    t.value = "";
  }

  async function pickWithTauriDialog() {
    if (!store.activeId) return;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({ multiple: true });
      if (!picked) return;
      const paths = Array.isArray(picked) ? picked : [picked];
      const valid = paths.filter((p): p is string => typeof p === "string");
      if (valid.length === 0) return;
      uploadError = null;
      uploading = true;
      try {
        const api = await store.ensureApi();
        if (!api.copyAttachment)
          throw new Error("copyAttachment not available in this transport");
        const added: Attachment[] = [];
        for (const p of valid) {
          const refPath = await api.copyAttachment(store.activeId, p);
          const name = p.split("/").pop() ?? p;
          added.push({ name, refPath });
        }
        attachments = [...attachments, ...added];
      } catch (e) {
        uploadError = String(e);
      } finally {
        uploading = false;
      }
    } catch (e) {
      uploadError = String(e);
    }
  }

  function removeAttachment(idx: number) {
    attachments = attachments.filter((_, i) => i !== idx);
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    dragging = false;
    if (e.dataTransfer?.files?.length) uploadFiles(e.dataTransfer.files);
  }

  function onDragOver(e: DragEvent) {
    e.preventDefault();
    dragging = true;
  }

  function onDragLeave(e: DragEvent) {
    // Only reset if leaving the root (not a child)
    if ((e.target as HTMLElement) === e.currentTarget) dragging = false;
  }

  async function runSlashCommand(cmd: SlashCommand) {
    const args = slashArgs;
    input = "";
    const api = await store.ensureApi();
    await cmd.run(
      {
        activeId: store.activeId,
        api,
        openSettings: () => onOpenSettings?.(),
        toggleRightPanel: () => onToggleRightPanel?.(),
        showHelp: () => (helpOpen = true),
        showDiff: () => openDiff(),
        showFileEditor: (title, tabs) => openFileEditor(title, tabs),
        showFileBrowser: (title, dirs) => openFileBrowser(title, dirs),
        showCost: () => openCost(),
        showContext: () => openContextStats(),
        showStats: () => openGlobalStats(),
        showModelPicker: () => openModelPicker(),
        showEffortPicker: () => openEffortPicker(),
        showAddDir: () => openAddDir(),
      },
      args,
    );
  }

  async function openFileEditor(
    title: string,
    tabs: { label: string; path: string; readonly?: boolean }[],
  ) {
    editorTitle = title;
    editorTabs = tabs.map((t) => ({
      ...t,
      content: "",
      loaded: false,
      dirty: false,
      error: null,
    }));
    editorActiveIdx = 0;
    editorSaveMsg = null;
    editorOpen = true;
    const api = await store.ensureApi();
    if (!api.fsRead) {
      editorTabs = editorTabs.map((t) => ({
        ...t,
        error: "File I/O only available in desktop mode",
        loaded: true,
      }));
      return;
    }
    for (let i = 0; i < editorTabs.length; i++) {
      const t = editorTabs[i];
      try {
        t.content = await api.fsRead(t.path);
      } catch (e) {
        t.error = String(e);
      } finally {
        t.loaded = true;
      }
    }
  }

  async function saveEditorTab() {
    const t = editorTabs[editorActiveIdx];
    if (!t || t.readonly) return;
    editorSaveMsg = null;
    try {
      const api = await store.ensureApi();
      if (!api.fsWrite) throw new Error("fsWrite not available");
      await api.fsWrite(t.path, t.content);
      t.dirty = false;
      editorSaveMsg = `Saved ${t.path}`;
      setTimeout(() => (editorSaveMsg = null), 2000);
    } catch (e) {
      t.error = String(e);
    }
  }

  async function openFileBrowser(
    title: string,
    dirs: { label: string; path: string }[],
  ) {
    browserTitle = title;
    browserOpen = true;
    browserSelectedPath = null;
    browserSelectedContent = "";
    browserSelectedError = null;
    const api = await store.ensureApi();
    const loaded: BrowserDir[] = [];
    for (const d of dirs) {
      try {
        if (!api.fsList) throw new Error("fsList not available");
        const entries = await api.fsList(d.path);
        loaded.push({
          label: d.label,
          path: d.path,
          entries: entries
            .filter((e) => !e.is_dir && e.name.endsWith(".md"))
            .map((e) => ({ name: e.name, path: e.path })),
          error: null,
        });
      } catch (e) {
        loaded.push({ label: d.label, path: d.path, entries: [], error: String(e) });
      }
    }
    browserDirs = loaded;
  }

  async function openBrowserEntry(path: string) {
    browserSelectedPath = path;
    browserSelectedContent = "";
    browserSelectedError = null;
    try {
      const api = await store.ensureApi();
      if (!api.fsRead) throw new Error("fsRead not available");
      browserSelectedContent = await api.fsRead(path);
    } catch (e) {
      browserSelectedError = String(e);
    }
  }

  async function openDiff() {
    diffOpen = true;
    diffError = null;
    diffLoading = true;
    diffText = "";
    try {
      const api = await store.ensureApi();
      if (!store.activeId) throw new Error("no active project");
      diffText = await api.gitDiff(store.activeId);
      if (!diffText.trim()) diffText = "(no uncommitted changes)";
    } catch (e) {
      diffError = String(e);
    } finally {
      diffLoading = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (slashVisible) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        slashSelectedIdx = (slashSelectedIdx + 1) % slashMatches.length;
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        slashSelectedIdx =
          (slashSelectedIdx - 1 + slashMatches.length) % slashMatches.length;
        return;
      }
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        const cmd = slashMatches[slashSelectedIdx];
        if (cmd) runSlashCommand(cmd);
        return;
      }
      if (e.key === "Tab") {
        e.preventDefault();
        const cmd = slashMatches[slashSelectedIdx];
        if (cmd) input = `/${cmd.name}`;
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        input = "";
        return;
      }
    }
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
    const count = st?.messages.length ?? 0;
    const appended = count > lastMessageCount;
    lastMessageCount = count;
    if (!scrollEl) return;
    if (appended && pinnedToBottom) {
      queueMicrotask(() => {
        if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
      });
    }
  });

  $effect(() => {
    const id = store.activeId;
    if (!id) return;
    untrack(() => {
      lastMessageCount = st?.messages.length ?? 0;
      pinnedToBottom = true;
    });
    queueMicrotask(() => {
      if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
    });
  });
</script>

<section
  class="relative flex-1 flex flex-col bg-bg-0 min-w-0 min-h-0"
  ondragover={onDragOver}
  ondragleave={onDragLeave}
  ondrop={onDrop}
>
  {#if dragging}
    <div
      class="absolute inset-4 z-30 rounded-2xl border-2 border-dashed border-accent bg-bg-0/80 flex items-center justify-center pointer-events-none"
    >
      <div class="text-accent text-sm font-medium">Drop files to attach</div>
    </div>
  {/if}
  {#if !active}
    <div class="flex-1 flex items-center justify-center text-text-3 text-[13px]">
      <p>Select or add a project to get started.</p>
    </div>
  {:else}
    <div
      class="flex-1 overflow-y-auto px-6 py-4"
      bind:this={scrollEl}
      onscroll={onScroll}
    >
      {#if st?.hasMoreHistory}
        <div class="text-center py-2">
          {#if st.loadingMore}
            <span class="text-text-3 text-[12px]">Loading older messages…</span>
          {:else}
            <button
              class="text-text-3 hover:text-text-1 text-[12px] underline bg-transparent border-none cursor-pointer"
              onclick={() => store.activeId && store.loadMoreHistory(store.activeId)}
            >
              Load older messages
            </button>
          {/if}
        </div>
      {/if}
      {#if st && st.messages.length === 0 && !st.hasMoreHistory}
        <div class="text-text-3 text-[13px] text-center py-10">
          Send your first message to Claude.
        </div>
      {/if}
      {#each st?.messages ?? [] as m (m.id)}
        <Message msg={m} />
      {/each}
    </div>

    <div class="shrink-0 p-4 border-t border-line bg-bg-1">
      {#if st?.contextTokens && st.contextTokens > 100_000 && !st.contextDismissed}
        <div
          class="mb-2 px-3 py-2 rounded-md border border-warn bg-[#2a1f10] text-text-1 text-[12px] flex items-start justify-between gap-3"
        >
          <span>
            This session's context is <strong>{Math.round(st.contextTokens / 1000)}k tokens</strong
            > — every new turn re-pays that as input. Consider running
            <code class="bg-bg-2 px-1 rounded">/compact</code> to summarize and start a leaner session.
          </span>
          <div class="flex items-center gap-1 shrink-0">
            <button
              type="button"
              onclick={() => {
                input = "/compact";
                queueMicrotask(() => textareaEl?.focus());
              }}
              class="bg-warn text-bg-1 border-none rounded px-2 py-0.5 text-[11px] font-medium cursor-pointer hover:brightness-110"
            >
              Run /compact
            </button>
            <button
              type="button"
              onclick={() => store.activeId && store.dismissContextWarning(store.activeId)}
              class="bg-transparent border-none text-text-3 hover:text-text-1 cursor-pointer text-lg leading-none px-1"
              aria-label="Dismiss"
            >
              ×
            </button>
          </div>
        </div>
      {/if}
      {#if st?.autoPaused}
        <div
          class="mb-2 px-3 py-2 rounded-md border border-line-2 bg-bg-2 text-text-2 text-[12px] flex items-start justify-between gap-3"
        >
          <span>
            Auto-paused after 5 min running in the background. Click
            <strong>Resume</strong> below, or abort if you don't need it.
          </span>
        </div>
      {/if}
      {#if sttError}
        <div
          class="mb-2 text-danger text-[12px] flex items-start justify-between gap-2"
        >
          <span>{sttError}</span>
          <button
            type="button"
            onclick={() => (sttError = null)}
            class="bg-transparent border-none text-text-3 hover:text-text-1 cursor-pointer"
            aria-label="Dismiss"
          >
            ×
          </button>
        </div>
      {/if}
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
          {#if slashVisible}
            <SlashPalette
              commands={slashMatches}
              selectedIdx={slashSelectedIdx}
              onPick={runSlashCommand}
            />
          {/if}
          {#if attachments.length > 0 || uploading || uploadError}
            <div class="flex flex-wrap gap-1.5 px-3 pt-2.5">
              {#each attachments as a, i (a.refPath)}
                <span
                  class="inline-flex items-center gap-1.5 bg-bg-1 border border-line text-text-1 text-[12px] rounded-full pl-2.5 pr-1 py-0.5"
                  title={a.refPath}
                >
                  <span class="truncate max-w-[180px]">{a.name}</span>
                  <button
                    type="button"
                    onclick={() => removeAttachment(i)}
                    aria-label="Remove attachment"
                    class="w-4 h-4 rounded-full bg-transparent border-none text-text-3 hover:text-danger cursor-pointer inline-flex items-center justify-center text-xs"
                  >
                    ×
                  </button>
                </span>
              {/each}
              {#if uploading}
                <span class="text-text-3 text-[12px] py-0.5">Uploading…</span>
              {/if}
              {#if uploadError}
                <span class="text-danger text-[12px] py-0.5" title={uploadError}
                  >Upload failed</span
                >
              {/if}
            </div>
          {/if}
          <textarea
            bind:this={textareaEl}
            placeholder={st?.busy
              ? "Claude is working…"
              : "Message Claude… (Enter to send, / for commands, Shift+Enter for new line)"}
            bind:value={input}
            onkeydown={onKey}
            disabled={st?.busy}
            rows="2"
            class="w-full bg-transparent border-none text-text-0 disabled:text-text-2 text-sm leading-relaxed resize-none outline-none pl-3.5 {sttAvailable
              ? 'pr-36'
              : 'pr-24'} py-3 min-h-[46px] max-h-[200px] font-sans"
          ></textarea>
          {#if sttAvailable}
            <button
              type="button"
              onclick={toggleRecording}
              disabled={st?.busy || transcribing}
              title={recording
                ? "Stop & transcribe"
                : transcribing
                  ? "Transcribing…"
                  : "Dictate (speech to text)"}
              aria-label={recording ? "Stop recording" : "Start dictation"}
              class="absolute right-[5.75rem] bottom-2 w-9 h-9 rounded-[10px] border border-line hover:text-text-0 hover:bg-bg-1 cursor-pointer inline-flex items-center justify-center bg-transparent disabled:opacity-40 disabled:cursor-not-allowed {recording
                ? 'text-danger border-danger animate-pulse'
                : 'text-text-1'}"
            >
              {#if transcribing}
                <svg
                  viewBox="0 0 24 24"
                  width="18"
                  height="18"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  class="animate-spin"
                >
                  <path d="M12 2a10 10 0 1 0 10 10" />
                </svg>
              {:else}
                <svg
                  viewBox="0 0 24 24"
                  width="18"
                  height="18"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <rect x="9" y="3" width="6" height="12" rx="3" />
                  <path d="M5 11a7 7 0 0 0 14 0" />
                  <path d="M12 18v3" />
                </svg>
              {/if}
            </button>
          {/if}
          {#if isTauri}
            <button
              type="button"
              onclick={pickWithTauriDialog}
              disabled={st?.busy || !store.activeId}
              title="Attach file"
              aria-label="Attach file"
              class="absolute right-12 bottom-2 w-9 h-9 rounded-[10px] border border-line text-text-1 hover:text-text-0 hover:bg-bg-1 cursor-pointer inline-flex items-center justify-center bg-transparent disabled:opacity-40 disabled:cursor-not-allowed"
            >
              <svg
                viewBox="0 0 24 24"
                width="18"
                height="18"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path
                  d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"
                />
              </svg>
            </button>
          {:else}
            <label
              title="Attach file"
              aria-label="Attach file"
              class="absolute right-12 bottom-2 w-9 h-9 rounded-[10px] border border-line text-text-1 hover:text-text-0 hover:bg-bg-1 cursor-pointer inline-flex items-center justify-center bg-transparent {st?.busy ||
              !store.activeId
                ? 'opacity-40 cursor-not-allowed pointer-events-none'
                : ''}"
            >
              <svg
                viewBox="0 0 24 24"
                width="18"
                height="18"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path
                  d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"
                />
              </svg>
              <input
                onchange={onFilePick}
                type="file"
                multiple
                disabled={st?.busy || !store.activeId}
                class="absolute inset-0 opacity-0 cursor-pointer disabled:cursor-not-allowed"
              />
            </label>
          {/if}
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
              disabled={!input.trim() && attachments.length === 0}
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

<SimpleModal bind:open={helpOpen} title="Slash commands">
  <div class="p-4">
    <p class="text-text-2 text-[12px] mb-3">
      Type <code class="bg-bg-2 border border-line px-1.5 py-0.5 rounded">/</code>
      in the composer to filter, then ↑↓ + Enter to run, or click an entry.
    </p>
    <ul class="list-none p-0 m-0 divide-y divide-line">
      {#each slashCommands as c (c.name)}
        <li class="py-2 flex items-baseline gap-3">
          <span class="text-accent font-mono text-[13px] shrink-0 min-w-32">
            /{c.name}{c.argHint ? ` ${c.argHint}` : ""}
          </span>
          <span class="text-text-2 text-[12px]">{c.description}</span>
        </li>
      {/each}
    </ul>
  </div>
</SimpleModal>

<SimpleModal bind:open={diffOpen} title="Git diff (uncommitted changes)">
  <div class="p-0">
    {#if diffLoading}
      <div class="text-text-3 text-[12px] p-4">Loading…</div>
    {:else if diffError}
      <div class="text-danger text-[12px] p-4">{diffError}</div>
    {:else}
      <pre
        class="m-0 p-4 text-[12px] leading-relaxed font-mono whitespace-pre overflow-x-auto text-text-1"
      >{diffText}</pre>
    {/if}
  </div>
</SimpleModal>

<SimpleModal bind:open={editorOpen} title={editorTitle}>
  <div class="flex flex-col h-full">
    {#if editorTabs.length > 1}
      <div class="shrink-0 flex border-b border-line bg-bg-2">
        {#each editorTabs as t, i (t.path)}
          <button
            type="button"
            onclick={() => (editorActiveIdx = i)}
            class="px-3 py-2 text-[12px] bg-transparent border-none cursor-pointer {i ===
            editorActiveIdx
              ? 'text-text-0 border-b-2 border-accent -mb-px'
              : 'text-text-2 hover:text-text-0'}"
          >
            {t.label}{t.dirty ? " •" : ""}
          </button>
        {/each}
      </div>
    {/if}
    {#if editorTabs[editorActiveIdx]}
      {@const t = editorTabs[editorActiveIdx]}
      <div class="flex-1 flex flex-col p-3 gap-2 min-h-[40vh]">
        <div class="text-[11px] text-text-3 font-mono break-all">{t.path}</div>
        {#if !t.loaded}
          <div class="text-text-3 text-[12px]">Loading…</div>
        {:else if t.error}
          <div class="text-danger text-[12px]">{t.error}</div>
        {:else}
          <textarea
            value={t.content}
            oninput={(e) => {
              t.content = (e.target as HTMLTextAreaElement).value;
              t.dirty = true;
            }}
            readonly={t.readonly}
            class="flex-1 w-full bg-bg-0 border border-line rounded-md p-3 text-[12.5px] font-mono text-text-1 resize-none outline-none focus:border-info min-h-[40vh]"
          ></textarea>
          {#if !t.readonly}
            <div class="flex items-center justify-between">
              <span class="text-[12px] text-text-3">{editorSaveMsg ?? ""}</span>
              <button
                type="button"
                onclick={saveEditorTab}
                disabled={!t.dirty}
                class="px-3 py-1.5 bg-accent hover:bg-accent-2 text-bg-1 border-none rounded-md text-[12px] cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
              >
                Save
              </button>
            </div>
          {/if}
        {/if}
      </div>
    {/if}
  </div>
</SimpleModal>

<SimpleModal bind:open={statsModalOpen} title={statsModalTitle}>
  <div class="p-4 flex flex-col gap-3">
    {#if statsError}
      <div class="text-danger text-[12px]">{statsError}</div>
    {:else if statsMode === "cost" && statsSession}
      {@const s = statsSession}
      <dl class="grid grid-cols-2 gap-x-6 gap-y-2 text-[13px] m-0">
        <dt class="text-text-3">Model</dt>
        <dd class="text-text-1 font-mono m-0">{s.model ?? "—"}</dd>
        <dt class="text-text-3">Input tokens</dt>
        <dd class="text-text-1 font-mono m-0">{formatTokens(s.input_tokens)}</dd>
        <dt class="text-text-3">Output tokens</dt>
        <dd class="text-text-1 font-mono m-0">{formatTokens(s.output_tokens)}</dd>
        <dt class="text-text-3">Cache writes</dt>
        <dd class="text-text-1 font-mono m-0">{formatTokens(s.cache_creation_tokens)}</dd>
        <dt class="text-text-3">Cache reads</dt>
        <dd class="text-text-1 font-mono m-0">{formatTokens(s.cache_read_tokens)}</dd>
        <dt class="text-text-3">Messages (user/assistant)</dt>
        <dd class="text-text-1 font-mono m-0">
          {s.user_messages} / {s.assistant_messages}
        </dd>
        <dt class="text-text-3">Estimated cost (USD)</dt>
        <dd class="text-accent font-mono m-0">
          ${estimateCost(s).toFixed(4)}
        </dd>
      </dl>
      <p class="text-[11px] text-text-3 m-0">
        Pricing is approximate; exact costs depend on the model version active
        when each turn was sent.
      </p>
    {:else if statsMode === "context" && statsSession}
      {@const s = statsSession}
      {@const cw = contextWindow(s.model)}
      {@const pct = cw > 0 ? (s.last_context_tokens / cw) * 100 : 0}
      <div class="flex flex-col gap-3 text-[13px]">
        <div class="text-text-3">
          {formatTokens(s.last_context_tokens)} / {formatTokens(cw)} tokens
          ({pct.toFixed(1)}%)
        </div>
        <div class="h-2 bg-bg-2 rounded-full overflow-hidden border border-line">
          <div
            class="h-full {pct > 80
              ? 'bg-danger'
              : pct > 50
                ? 'bg-warn'
                : 'bg-ok'}"
            style="width: {Math.min(100, pct).toFixed(2)}%"
          ></div>
        </div>
        <p class="text-[11px] text-text-3 m-0">
          Based on the last assistant turn's <code>input + cache_read + cache_creation</code>.
          Model: <code class="text-text-1">{s.model ?? "unknown"}</code>. Window is
          estimated ({formatTokens(cw)}). Use <code>/compact</code> to free up space.
        </p>
      </div>
    {:else if statsMode === "stats" && statsGlobal}
      {@const g = statsGlobal}
      <dl class="grid grid-cols-2 gap-x-6 gap-y-2 text-[13px] m-0">
        <dt class="text-text-3">Sessions</dt>
        <dd class="text-text-1 font-mono m-0">{g.total_sessions}</dd>
        <dt class="text-text-3">Input tokens</dt>
        <dd class="text-text-1 font-mono m-0">
          {formatTokens(g.total_input_tokens)}
        </dd>
        <dt class="text-text-3">Output tokens</dt>
        <dd class="text-text-1 font-mono m-0">
          {formatTokens(g.total_output_tokens)}
        </dd>
        <dt class="text-text-3">Cache writes / reads</dt>
        <dd class="text-text-1 font-mono m-0">
          {formatTokens(g.total_cache_creation_tokens)} / {formatTokens(
            g.total_cache_read_tokens,
          )}
        </dd>
      </dl>
      {#if g.by_project.length > 0}
        <h3
          class="text-[11px] uppercase tracking-wider text-text-3 font-semibold mt-3 mb-1"
        >
          By project
        </h3>
        <table class="w-full text-[12px] border border-line">
          <thead class="bg-bg-2">
            <tr>
              <th class="text-left px-2 py-1 text-text-2 font-semibold">Project</th>
              <th class="text-right px-2 py-1 text-text-2 font-semibold">Sessions</th>
              <th class="text-right px-2 py-1 text-text-2 font-semibold">In</th>
              <th class="text-right px-2 py-1 text-text-2 font-semibold">Out</th>
            </tr>
          </thead>
          <tbody>
            {#each g.by_project as p (p.name)}
              <tr class="border-t border-line">
                <td class="px-2 py-1 text-text-1">{p.name}</td>
                <td class="px-2 py-1 text-text-1 text-right font-mono">{p.sessions}</td>
                <td class="px-2 py-1 text-text-1 text-right font-mono">
                  {formatTokens(p.input_tokens)}
                </td>
                <td class="px-2 py-1 text-text-1 text-right font-mono">
                  {formatTokens(p.output_tokens)}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {:else}
      <div class="text-text-3 text-[12px]">Loading…</div>
    {/if}
  </div>
</SimpleModal>

<SimpleModal bind:open={flagsModalOpen} title={flagsModalTitle}>
  <div class="p-4 flex flex-col gap-3">
    {#if !flagsCurrent}
      <div class="text-text-3 text-[12px]">Loading…</div>
    {:else if flagsModalKind === "model"}
      <p class="text-[12px] text-text-3 m-0">
        Current: <code class="text-text-1">{flagsCurrent.model ?? "(default)"}</code>
      </p>
      <div class="flex flex-col gap-1">
        {#each MODEL_OPTIONS as opt (opt.value)}
          <button
            type="button"
            onclick={() => pickFlag(opt.value)}
            class="text-left px-3 py-2 rounded-md text-[13px] border {(flagsCurrent?.model ??
              '') === opt.value
              ? 'border-accent bg-bg-2 text-text-0'
              : 'border-line bg-transparent text-text-1 hover:bg-bg-2'} cursor-pointer"
          >
            <span class="font-mono text-accent mr-2">{opt.value || "—"}</span>
            <span class="text-text-2">{opt.label}</span>
          </button>
        {/each}
      </div>
    {:else if flagsModalKind === "effort"}
      <p class="text-[12px] text-text-3 m-0">
        Current: <code class="text-text-1">{flagsCurrent.effort ?? "(default)"}</code>
      </p>
      <div class="flex flex-col gap-1">
        {#each EFFORT_OPTIONS as opt (opt.value)}
          <button
            type="button"
            onclick={() => pickFlag(opt.value)}
            class="text-left px-3 py-2 rounded-md text-[13px] border {(flagsCurrent?.effort ??
              '') === opt.value
              ? 'border-accent bg-bg-2 text-text-0'
              : 'border-line bg-transparent text-text-1 hover:bg-bg-2'} cursor-pointer"
          >
            <span class="font-mono text-accent mr-2">{opt.value || "—"}</span>
            <span class="text-text-2">{opt.label}</span>
          </button>
        {/each}
      </div>
    {:else if flagsModalKind === "add-dir"}
      <div class="flex gap-2">
        <input
          type="text"
          placeholder="/absolute/path/to/dir"
          bind:value={flagsNewDir}
          class="flex-1 bg-bg-0 border border-line rounded-md px-3 py-2 text-[13px] text-text-0 outline-none focus:border-info"
        />
        <button
          type="button"
          onclick={addDir}
          disabled={!flagsNewDir.trim()}
          class="px-3 py-2 bg-accent hover:bg-accent-2 text-bg-1 border-none rounded-md text-[12px] cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
        >
          Add
        </button>
      </div>
      {#if flagsCurrent.add_dirs.length === 0}
        <p class="text-[12px] text-text-3 m-0 italic">No extra directories.</p>
      {:else}
        <ul class="list-none p-0 m-0 border border-line rounded-md overflow-hidden">
          {#each flagsCurrent.add_dirs as p (p)}
            <li class="flex items-center justify-between px-3 py-2 border-b border-line last:border-b-0">
              <span class="text-[12px] font-mono text-text-1 truncate">{p}</span>
              <button
                type="button"
                onclick={() => removeDir(p)}
                aria-label="Remove"
                class="bg-transparent border-none text-text-3 hover:text-danger cursor-pointer w-6 h-6 text-lg"
              >
                ×
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  </div>
</SimpleModal>

<SimpleModal bind:open={browserOpen} title={browserTitle}>
  <div class="flex flex-col min-h-[40vh]">
    {#if browserDirs.length === 0}
      <div class="text-text-3 text-[12px] p-4">Loading…</div>
    {:else}
      <div class="p-3 flex flex-col gap-3">
        {#each browserDirs as d (d.path)}
          <section>
            <header class="mb-1.5 flex items-baseline justify-between gap-2">
              <h3 class="text-[11px] uppercase tracking-wider text-text-3 font-semibold m-0">
                {d.label}
              </h3>
              <span class="text-[11px] text-text-3 font-mono truncate">{d.path}</span>
            </header>
            {#if d.error}
              <div class="text-danger text-[12px]">{d.error}</div>
            {:else if d.entries.length === 0}
              <div class="text-text-3 text-[12px] italic">(empty)</div>
            {:else}
              <ul class="list-none p-0 m-0 border border-line rounded-md overflow-hidden">
                {#each d.entries as e (e.path)}
                  <button
                    type="button"
                    onclick={() => openBrowserEntry(e.path)}
                    class="block w-full text-left px-3 py-1.5 bg-transparent border-none cursor-pointer text-[12.5px] text-text-1 hover:bg-bg-2 {browserSelectedPath ===
                    e.path
                      ? 'bg-bg-2'
                      : ''}"
                  >
                    {e.name}
                  </button>
                {/each}
              </ul>
            {/if}
          </section>
        {/each}
        {#if browserSelectedPath}
          <section class="mt-2 border-t border-line pt-3">
            <div class="text-[11px] text-text-3 font-mono break-all mb-2">
              {browserSelectedPath}
            </div>
            {#if browserSelectedError}
              <div class="text-danger text-[12px]">{browserSelectedError}</div>
            {:else}
              <pre
                class="m-0 p-3 bg-bg-0 border border-line rounded-md text-[12px] font-mono text-text-1 max-h-[40vh] overflow-auto whitespace-pre-wrap"
              >{browserSelectedContent}</pre>
            {/if}
          </section>
        {/if}
      </div>
    {/if}
  </div>
</SimpleModal>
