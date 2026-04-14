use anyhow::Result;
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

/// Load a session's transcript as a vec of JSON events (one per line).
pub fn load_session(project_path: &str, session_id: &str) -> Result<Vec<Value>> {
    let dir = match encoded_project_dir(project_path) {
        Some(d) => d,
        None => return Ok(vec![]),
    };
    let file = dir.join(format!("{}.jsonl", session_id));
    if !file.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(file)?;
    let events = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    Ok(events)
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
