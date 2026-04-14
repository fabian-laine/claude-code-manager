export type Project = {
  id: string;
  name: string;
  path: string;
  created_at: number;
  last_session_id: string | null;
};

export type ClaudeEvent =
  | { kind: "started"; project_id: string }
  | { kind: "raw"; project_id: string; event: any }
  | { kind: "finished"; project_id: string; session_id: string | null }
  | { kind: "error"; project_id: string; message: string }
  | { kind: "cancelled"; project_id: string }
  | { kind: "paused"; project_id: string }
  | { kind: "resumed"; project_id: string };

export type RenderedMessage =
  | { type: "user"; text: string; id: string }
  | { type: "assistant_text"; text: string; id: string }
  | { type: "thinking"; text: string; id: string }
  | { type: "tool_use"; name: string; input: any; id: string }
  | { type: "tool_result"; content: string; is_error: boolean; tool_use_id: string; id: string }
  | { type: "system"; text: string; id: string }
  | { type: "error"; text: string; id: string };
