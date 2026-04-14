<script lang="ts">
  import { marked } from "marked";
  import hljs from "highlight.js";

  let { source }: { source: string } = $props();

  marked.setOptions({
    gfm: true,
    breaks: true,
  });

  // @ts-ignore — marked v12+ extension API
  marked.use({
    renderer: {
      code({ text, lang }: { text: string; lang?: string }) {
        const language = lang && hljs.getLanguage(lang) ? lang : "plaintext";
        let highlighted: string;
        try {
          highlighted = hljs.highlight(text, { language, ignoreIllegals: true }).value;
        } catch {
          highlighted = escapeHtml(text);
        }
        return `<pre class="hljs"><code class="language-${language}">${highlighted}</code></pre>`;
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
</script>

<div class="md">{@html html}</div>

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
