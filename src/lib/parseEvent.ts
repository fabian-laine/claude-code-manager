import type { RenderedMessage } from "./types";

let counter = 0;
const nid = () => `m${++counter}-${Date.now()}`;

/**
 * Convert a raw Claude Code stream-json event into zero or more displayable messages.
 */
export function parseClaudeEvent(ev: any): RenderedMessage[] {
  if (!ev || typeof ev !== "object") return [];
  const out: RenderedMessage[] = [];
  const type = ev.type;

  if (type === "system") {
    return out;
  }

  if (type === "assistant") {
    const msg = ev.message;
    const content = msg?.content ?? [];
    for (const block of content) {
      if (block.type === "text" && typeof block.text === "string") {
        out.push({ type: "assistant_text", text: block.text, id: block.id ?? nid() });
      } else if (block.type === "thinking" && typeof block.thinking === "string") {
        out.push({ type: "thinking", text: block.thinking, id: nid() });
      } else if (block.type === "tool_use") {
        out.push({
          type: "tool_use",
          name: block.name ?? "tool",
          input: block.input ?? {},
          id: block.id ?? nid(),
        });
      }
    }
    return out;
  }

  if (type === "user") {
    const msg = ev.message;
    const content = msg?.content ?? [];
    for (const block of content) {
      if (block.type === "tool_result") {
        let text = "";
        if (typeof block.content === "string") text = block.content;
        else if (Array.isArray(block.content)) {
          text = block.content
            .map((c: any) => (typeof c === "string" ? c : c.text ?? ""))
            .join("\n");
        }
        out.push({
          type: "tool_result",
          content: text,
          is_error: !!block.is_error,
          tool_use_id: block.tool_use_id ?? "",
          id: nid(),
        });
      } else if (block.type === "text" && typeof block.text === "string") {
        out.push({ type: "user", text: block.text, id: nid() });
      }
    }
    return out;
  }

  if (type === "result") {
    // Final turn marker — some runs deliver the final answer here (string `result` field)
    // rather than in a preceding assistant text block. Emit it if present and non-empty.
    if (typeof ev.result === "string" && ev.result.trim().length > 0) {
      out.push({ type: "assistant_text", text: ev.result, id: `result-${nid()}` });
    }
    return out;
  }

  return out;
}
