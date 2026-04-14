<script lang="ts">
  import { marked } from "marked";
  import hljs from "highlight.js";
  import mermaid from "mermaid";

  let { source }: { source: string } = $props();

  mermaid.initialize({
    startOnLoad: false,
    theme: "dark",
    securityLevel: "loose",
    fontFamily: "inherit",
    themeVariables: {
      darkMode: true,
      background: "#0e0e12",
      primaryColor: "#1a1a22",
      primaryTextColor: "#e8e8ee",
      primaryBorderColor: "#3a3a45",
      lineColor: "#6a6a75",
      secondaryColor: "#26262d",
      tertiaryColor: "#14141a",
    },
  });

  let mermaidCounter = 0;

  marked.setOptions({
    gfm: true,
    breaks: true,
  });

  function looksLikeMermaid(text: string): boolean {
    const first = text.trimStart().split("\n", 1)[0] ?? "";
    return /^(flowchart|graph|sequenceDiagram|classDiagram|stateDiagram(-v2)?|erDiagram|journey|gantt|pie|mindmap|timeline|quadrantChart|requirementDiagram|gitGraph|C4Context|C4Container|C4Component|C4Dynamic|C4Deployment|xychart-beta|sankey-beta|block-beta|packet-beta|architecture-beta)\b/i.test(
      first,
    );
  }

  // @ts-ignore — marked v12+ extension API
  marked.use({
    renderer: {
      code({ text, lang }: { text: string; lang?: string }) {
        const isMermaid = lang === "mermaid" || (!lang && looksLikeMermaid(text));
        const language = isMermaid
          ? "mermaid"
          : lang && hljs.getLanguage(lang)
            ? lang
            : "plaintext";
        let highlighted: string;
        if (isMermaid) {
          // Mermaid isn't in hljs; render the source plain.
          highlighted = escapeHtml(text);
        } else {
          try {
            highlighted = hljs.highlight(text, { language, ignoreIllegals: true }).value;
          } catch {
            highlighted = escapeHtml(text);
          }
        }
        const raw = escapeHtml(text);
        const mermaidAttr = isMermaid ? ' data-mermaid="true"' : "";
        return `<pre class="hljs" data-code="${raw}"${mermaidAttr}><code class="language-${language}">${highlighted}</code></pre>`;
      },
    },
  });

  function escapeHtml(s: string): string {
    return s
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  const html = $derived(marked.parse(source) as string);

  let container: HTMLDivElement | undefined = $state();

  async function renderMermaid(pre: HTMLPreElement) {
    if (pre.previousElementSibling?.classList.contains("mermaid-diagram")) return;
    const raw = pre.getAttribute("data-code") ?? "";
    const source = raw
      .replace(/&amp;/g, "&")
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .replace(/&quot;/g, '"')
      .replace(/&#39;/g, "'");
    const wrapper = document.createElement("div");
    wrapper.className = "mermaid-diagram";
    wrapper.textContent = "Rendering diagram…";
    pre.parentNode?.insertBefore(wrapper, pre);
    try {
      const id = `mermaid-${++mermaidCounter}-${Date.now()}`;
      const { svg } = await mermaid.render(id, source);
      wrapper.innerHTML = svg;
    } catch (e) {
      wrapper.className = "mermaid-diagram mermaid-error";
      wrapper.textContent = `Mermaid error: ${(e as Error).message ?? String(e)}`;
    }
  }

  $effect(() => {
    // Re-run when the rendered HTML changes.
    const _ = html;
    if (!container) return;
    queueMicrotask(() => {
      if (!container) return;
      const pres = container.querySelectorAll<HTMLPreElement>("pre.hljs");
      pres.forEach((pre) => {
        if (pre.hasAttribute("data-mermaid")) {
          renderMermaid(pre);
        }
        if (pre.querySelector(".copy-btn")) return;
        pre.style.position = "relative";
        const btn = document.createElement("button");
        btn.className = "copy-btn";
        btn.type = "button";
        btn.title = "Copy";
        btn.setAttribute("aria-label", "Copy code");
        btn.innerHTML =
          '<svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M16 1H4a2 2 0 0 0-2 2v14h2V3h12V1zm3 4H8a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2zm0 16H8V7h11v14z"/></svg>';
        btn.addEventListener("click", async (e) => {
          e.stopPropagation();
          const raw = pre.getAttribute("data-code") ?? "";
          // Un-escape the HTML entities we stored.
          const txt = raw
            .replace(/&amp;/g, "&")
            .replace(/&lt;/g, "<")
            .replace(/&gt;/g, ">")
            .replace(/&quot;/g, '"')
            .replace(/&#39;/g, "'");
          try {
            await navigator.clipboard.writeText(txt);
            btn.classList.add("copied");
            btn.innerHTML =
              '<svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41L9 16.17z"/></svg>';
            setTimeout(() => {
              btn.classList.remove("copied");
              btn.innerHTML =
                '<svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><path d="M16 1H4a2 2 0 0 0-2 2v14h2V3h12V1zm3 4H8a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2zm0 16H8V7h11v14z"/></svg>';
            }, 1500);
          } catch (err) {
            console.error("copy failed", err);
          }
        });
        pre.appendChild(btn);
      });
    });
  });
</script>

<div class="md" bind:this={container}>{@html html}</div>

<style>
  .md {
    color: #e8e8ee;
    line-height: 1.6;
    font-size: 14px;
  }
  .md :global(p) {
    margin: 0.5em 0;
  }
  .md :global(p:first-child) {
    margin-top: 0;
  }
  .md :global(p:last-child) {
    margin-bottom: 0;
  }
  .md :global(h1),
  .md :global(h2),
  .md :global(h3),
  .md :global(h4) {
    color: #f0f0f5;
    margin: 1em 0 0.4em 0;
    font-weight: 600;
    line-height: 1.3;
  }
  .md :global(h1) { font-size: 1.5em; }
  .md :global(h2) { font-size: 1.3em; }
  .md :global(h3) { font-size: 1.15em; }
  .md :global(h4) { font-size: 1em; }
  .md :global(a) {
    color: #6aa0d0;
    text-decoration: none;
  }
  .md :global(a:hover) {
    text-decoration: underline;
  }
  .md :global(ul),
  .md :global(ol) {
    padding-left: 1.4em;
    margin: 0.4em 0;
  }
  .md :global(li) {
    margin: 0.15em 0;
  }
  .md :global(code) {
    background: #1a1a22;
    border: 1px solid #26262d;
    border-radius: 4px;
    padding: 1px 6px;
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 0.88em;
    color: #c9a96e;
  }
  .md :global(pre) {
    background: #0b0b10;
    border: 1px solid #26262d;
    border-radius: 8px;
    padding: 12px 14px;
    overflow-x: auto;
    margin: 0.7em 0;
  }
  .md :global(pre .copy-btn) {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 28px;
    height: 28px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: #1a1a22;
    border: 1px solid #2a2a33;
    border-radius: 6px;
    color: #9a9aa5;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.15s, color 0.15s, background 0.15s;
    padding: 0;
  }
  .md :global(pre:hover .copy-btn) {
    opacity: 1;
  }
  .md :global(pre .copy-btn:hover) {
    background: #24242e;
    color: #e8e8ee;
  }
  .md :global(pre .copy-btn.copied) {
    color: #7aa870;
    opacity: 1;
  }
  .md :global(.mermaid-diagram) {
    background: #0b0b10;
    border: 1px solid #26262d;
    border-radius: 8px;
    padding: 16px;
    margin: 0.7em 0;
    overflow-x: auto;
    text-align: center;
    color: #9a9aa5;
    font-size: 12px;
  }
  .md :global(.mermaid-diagram svg) {
    max-width: 100%;
    height: auto;
  }
  .md :global(.mermaid-diagram.mermaid-error) {
    color: #f87171;
    border-color: #5a2a2a;
    background: #1a1010;
    text-align: left;
    font-family: "JetBrains Mono", ui-monospace, monospace;
  }
  .md :global(pre code) {
    background: transparent;
    border: none;
    padding: 0;
    color: #c8c8d2;
    font-size: 12.5px;
    line-height: 1.55;
  }
  .md :global(blockquote) {
    border-left: 3px solid #3a3a45;
    padding: 0.1em 0 0.1em 1em;
    margin: 0.7em 0;
    color: #9a9aa5;
  }
  .md :global(table) {
    border-collapse: collapse;
    margin: 0.7em 0;
  }
  .md :global(th),
  .md :global(td) {
    border: 1px solid #26262d;
    padding: 6px 10px;
    text-align: left;
  }
  .md :global(th) {
    background: #16161c;
    color: #f0f0f5;
    font-weight: 600;
  }
  .md :global(hr) {
    border: none;
    border-top: 1px solid #26262d;
    margin: 1em 0;
  }
  .md :global(strong) {
    color: #f0f0f5;
    font-weight: 600;
  }
  .md :global(em) {
    color: #d8d8e2;
  }

  /* highlight.js tokens — dark theme */
  .md :global(.hljs-comment),
  .md :global(.hljs-quote) {
    color: #6a6a75;
    font-style: italic;
  }
  .md :global(.hljs-keyword),
  .md :global(.hljs-selector-tag),
  .md :global(.hljs-literal),
  .md :global(.hljs-section),
  .md :global(.hljs-link) {
    color: #c586c0;
  }
  .md :global(.hljs-string),
  .md :global(.hljs-regexp),
  .md :global(.hljs-addition),
  .md :global(.hljs-attribute),
  .md :global(.hljs-meta-string) {
    color: #ce9178;
  }
  .md :global(.hljs-number),
  .md :global(.hljs-built_in),
  .md :global(.hljs-literal),
  .md :global(.hljs-type),
  .md :global(.hljs-params) {
    color: #b5cea8;
  }
  .md :global(.hljs-title),
  .md :global(.hljs-title.function_),
  .md :global(.hljs-name) {
    color: #dcdcaa;
  }
  .md :global(.hljs-variable),
  .md :global(.hljs-template-variable),
  .md :global(.hljs-attr) {
    color: #9cdcfe;
  }
  .md :global(.hljs-tag) {
    color: #569cd6;
  }
  .md :global(.hljs-symbol),
  .md :global(.hljs-bullet),
  .md :global(.hljs-subst),
  .md :global(.hljs-meta),
  .md :global(.hljs-selector-attr),
  .md :global(.hljs-selector-pseudo) {
    color: #d7ba7d;
  }
  .md :global(.hljs-deletion) {
    color: #f87171;
  }
</style>
