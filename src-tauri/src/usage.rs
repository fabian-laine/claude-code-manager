use anyhow::{anyhow, Result};
use serde_json::Value;

/// Python script that spawns `claude` on a PTY, types `/usage`, captures
/// the TUI output, and prints parsed JSON on stdout.
const PY_SCRIPT: &str = include_str!("claude_usage.py");

/// Fetch Claude Code's plan-usage snapshot by shelling out to the embedded
/// Python capture script. Relies on `python3` being on PATH (standard on
/// Linux).
pub async fn fetch_claude_usage() -> Result<Value> {
    let out = tokio::process::Command::new("python3")
        .arg("-c")
        .arg(PY_SCRIPT)
        .output()
        .await
        .map_err(|e| anyhow!("failed to launch python3: {}", e))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("claude_usage.py exited {}: {}", out.status, stderr));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let trimmed = stdout.trim();
    serde_json::from_str::<Value>(trimmed)
        .map_err(|e| anyhow!("invalid JSON from claude_usage.py: {} — got: {}", e, trimmed))
}
