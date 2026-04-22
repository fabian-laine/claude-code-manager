use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

/// Claude Code stores session transcripts at
/// ~/.claude/projects/<encoded-path>/<session-id>.jsonl
/// Encoding replaces '/' with '-'.
pub fn encoded_project_dir(project_path: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let abs = fs::canonicalize(project_path).ok()?;
    let encoded = abs.to_string_lossy().replace('/', "-");
    Some(home.join(".claude").join("projects").join(encoded))
}

#[derive(Serialize, Debug, Clone)]
pub struct HistoryChunk {
    pub events: Vec<Value>,
    pub oldest_ts: Option<String>,
    pub has_more: bool,
    pub session_id: Option<String>,
}

fn empty_chunk(session_id: Option<String>) -> HistoryChunk {
    HistoryChunk {
        events: vec![],
        oldest_ts: None,
        has_more: false,
        session_id,
    }
}

/// An event marks the start of a new "turn" when it's a user text message
/// (not a tool_result continuation of a previous assistant turn).
fn is_turn_start(v: &Value) -> bool {
    if v.get("type").and_then(|t| t.as_str()) != Some("user") {
        return false;
    }
    let Some(msg) = v.get("message") else {
        return false;
    };
    let Some(content) = msg.get("content") else {
        return false;
    };
    if content.is_string() {
        return true;
    }
    if let Some(arr) = content.as_array() {
        return arr
            .iter()
            .any(|b| b.get("type").and_then(|t| t.as_str()) != Some("tool_result"));
    }
    false
}

/// Load events from a session's transcript in a time-bounded chunk.
///
/// `before_ts` is an exclusive upper-bound cursor. `None` means "start from the newest events".
/// The function returns events in `[window_end - hours, window_end)`, expanded backwards
/// if needed so the oldest event is always a turn start (no mid-turn splits).
/// If the initial window is empty it keeps sliding backwards by `hours` until it either
/// finds events or runs out of older events.
pub fn load_session_chunk(
    project_path: &str,
    session_id: &str,
    before_ts: Option<&str>,
    hours: u32,
) -> Result<HistoryChunk> {
    let dir = match encoded_project_dir(project_path) {
        Some(d) => d,
        None => return Ok(empty_chunk(None)),
    };
    let file = dir.join(format!("{}.jsonl", session_id));
    if !file.exists() {
        return Ok(empty_chunk(Some(session_id.to_string())));
    }
    let content = fs::read_to_string(&file)?;

    let mut all: Vec<(DateTime<Utc>, Value)> = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(ts_str) = v.get("timestamp").and_then(|t| t.as_str()) else {
            continue;
        };
        let Ok(ts) = DateTime::parse_from_rfc3339(ts_str) else {
            continue;
        };
        all.push((ts.with_timezone(&Utc), v));
    }

    if all.is_empty() {
        return Ok(empty_chunk(Some(session_id.to_string())));
    }

    all.sort_by(|a, b| a.0.cmp(&b.0));

    let end_cursor = match before_ts {
        Some(s) => DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| all.last().unwrap().0 + Duration::seconds(1)),
        None => all.last().unwrap().0 + Duration::seconds(1),
    };

    let window_size = Duration::hours(hours.max(1) as i64);
    let mut window_end = end_cursor;

    loop {
        let window_start = window_end - window_size;
        let first_idx = all
            .iter()
            .position(|(t, _)| *t >= window_start && *t < window_end);

        if let Some(mut idx) = first_idx {
            // Respect turn boundaries: back up until the first event is a turn start.
            // We only back up within the file — never beyond the very first event.
            while idx > 0 && !is_turn_start(&all[idx].1) {
                idx -= 1;
            }

            let mut events = Vec::new();
            let mut oldest = all[idx].0;
            for (t, v) in &all[idx..] {
                if *t >= window_end {
                    break;
                }
                events.push(v.clone());
                if *t < oldest {
                    oldest = *t;
                }
            }

            let has_more = all.iter().any(|(t, _)| *t < oldest);

            return Ok(HistoryChunk {
                events,
                oldest_ts: Some(oldest.to_rfc3339()),
                has_more,
                session_id: Some(session_id.to_string()),
            });
        }

        if !all.iter().any(|(t, _)| *t < window_start) {
            return Ok(empty_chunk(Some(session_id.to_string())));
        }

        window_end = window_start;
    }
}

/// Find the most recent session id for a project by inspecting mtime.
pub fn latest_session_id(project_path: &str) -> Option<String> {
    let dir = encoded_project_dir(project_path)?;
    if !dir.exists() {
        return None;
    }
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else { continue };
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if newest.as_ref().map_or(true, |(t, _)| modified > *t) {
            newest = Some((modified, stem.to_string()));
        }
    }
    newest.map(|(_, id)| id)
}
