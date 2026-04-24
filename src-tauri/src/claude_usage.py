"""
Capture Claude Code's /usage output by spawning the CLI on a pseudo-tty,
typing /usage, then reading and parsing the TUI output.

This is intentionally fragile — /usage is a TUI-rendered view, not an API.
If Claude Code changes its output format we'll need to adjust regexes.

Outputs a single JSON line on stdout: the parsed usage, or {"error": "..."}
on failure.
"""
import fcntl
import json
import os
import pty
import re
import select
import signal
import struct
import sys
import termios
import time

ANSI_RE = re.compile(rb"\x1B\[[0-?]*[ -/]*[@-~]|\x1B\][^\x07]*\x07|\x1B[=><]")


def resolve_claude() -> str:
    """Find the `claude` binary. Desktop-launched apps on Linux often don't
    have ~/.local/bin in PATH, so fall back to common install locations."""
    home = os.path.expanduser("~")
    candidates = [
        os.path.join(home, ".local/bin/claude"),
        os.path.join(home, ".claude/local/claude"),
        os.path.join(home, ".bun/bin/claude"),
        os.path.join(home, ".npm-global/bin/claude"),
        "/usr/local/bin/claude",
        "/usr/bin/claude",
        "/opt/homebrew/bin/claude",
    ]
    # PATH lookup first (cheap), then fallback probes.
    from shutil import which
    found = which("claude")
    if found:
        return found
    for p in candidates:
        if os.path.isfile(p) and os.access(p, os.X_OK):
            return p
    return "claude"  # last-ditch — execvp will fail with a useful error


def capture(stats: dict) -> bytes:
    """Capture the TUI output. `stats` is mutated with per-phase timing/bytes
    so the caller can surface diagnostics when parsing fails."""
    claude_bin = resolve_claude()
    stats["claude_bin"] = claude_bin
    pid, fd = pty.fork()
    if pid == 0:
        # child
        try:
            os.execv(claude_bin, ["claude"])
        except OSError as e:
            sys.stderr.write(f"execv({claude_bin}) failed: {e}\n")
            os._exit(127)
    try:
        # When spawned without a controlling terminal (e.g. from the Tauri
        # subprocess), the PTY defaults to 0x0 and Claude's TUI renders
        # collapsed — section headers never appear. Force a reasonable size.
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))
        # Wait for claude to finish booting (MCP plugins can delay this by
        # several seconds — e.g. claude-mem fetches memory over localhost).
        # Exit early after 1.2s of silence, hard cap at 20s.
        t0 = time.time()
        boot = drain_until_idle(fd, max_wait=20.0, quiet_ms=1200)
        stats["boot_ms"] = int((time.time() - t0) * 1000)
        stats["boot_bytes"] = len(boot)
        # Dismiss any pending popup/dialog, then clear any stray input.
        os.write(fd, b"\x1b")
        time.sleep(0.15)
        os.write(fd, b"\x15")
        time.sleep(0.1)
        # Type `/usage` at once and submit.
        os.write(fd, b"/usage")
        time.sleep(0.4)
        os.write(fd, b"\r")
        # Drain the response, early-exit once we see the usage markers.
        t1 = time.time()
        resp = drain_until_markers(fd, max_wait=15.0, quiet_ms=1500)
        stats["response_ms"] = int((time.time() - t1) * 1000)
        stats["response_bytes"] = len(resp)
        # Send exit so claude cleans up its terminal state.
        try:
            os.write(fd, b"/exit\r")
        except OSError:
            pass
        return boot + resp
    finally:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            os.close(fd)
        except OSError:
            pass


def drain(fd: int, seconds: float) -> bytes:
    out = b""
    deadline = time.time() + seconds
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.3)
        if not r:
            continue
        try:
            chunk = os.read(fd, 16384)
        except OSError:
            break
        if not chunk:
            break
        out += chunk
    return out


def drain_until_idle(fd: int, max_wait: float, quiet_ms: int) -> bytes:
    """Read until `quiet_ms` elapses with no new bytes, or `max_wait` expires."""
    out = b""
    deadline = time.time() + max_wait
    last_activity = time.time()
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.2)
        if r:
            try:
                chunk = os.read(fd, 16384)
            except OSError:
                break
            if not chunk:
                break
            out += chunk
            last_activity = time.time()
        elif (time.time() - last_activity) * 1000 >= quiet_ms:
            break
    return out


_USAGE_MARKERS = re.compile(rb"%\s*used|Resets?\s", flags=re.IGNORECASE)


def drain_until_markers(fd: int, max_wait: float, quiet_ms: int) -> bytes:
    """Like drain_until_idle, but exits as soon as usage markers appear AND
    a short quiet period follows (so we don't cut off mid-render)."""
    out = b""
    deadline = time.time() + max_wait
    last_activity = time.time()
    saw_markers_at = None
    while time.time() < deadline:
        r, _, _ = select.select([fd], [], [], 0.2)
        if r:
            try:
                chunk = os.read(fd, 16384)
            except OSError:
                break
            if not chunk:
                break
            out += chunk
            last_activity = time.time()
            if saw_markers_at is None and _USAGE_MARKERS.search(ANSI_RE.sub(b"", out)):
                saw_markers_at = time.time()
        else:
            quiet_for = (time.time() - last_activity) * 1000
            # Once the page rendered, 800ms of silence is enough.
            if saw_markers_at is not None and quiet_for >= 800:
                break
            # Nothing yet: bail after `quiet_ms` of dead air.
            if saw_markers_at is None and quiet_for >= quiet_ms:
                break
    return out


def find_section(text: str, marker_pattern: str) -> dict | None:
    """Locate a section by a regex pattern (since the TUI strips whitespace
    inconsistently between words) and extract its first percent + resets line."""
    m = re.search(marker_pattern, text, flags=re.IGNORECASE)
    if not m:
        return None
    section = text[m.end() : m.end() + 500]
    pct = re.search(r"(\d+)\s*%\s*used", section, flags=re.IGNORECASE)
    # TUI drops the space after "Resets" ("Resets4pm…"), so no \s+ required.
    reset = re.search(
        r"Resets?\s*([^\r\n─│]+?)(?:\s{2,}|[\r\n]|$)",
        section,
        flags=re.IGNORECASE,
    )
    if not pct:
        return None
    return {
        "percent": int(pct.group(1)),
        "resets": reset.group(1).strip() if reset else None,
    }


DEBUG_LOG = "/tmp/ccm-usage-debug.log"


def write_debug(raw: bytes, cleaned: str, stats: dict) -> None:
    try:
        with open(DEBUG_LOG, "wb") as f:
            f.write(f"=== stats ===\n{json.dumps(stats, indent=2)}\n".encode())
            f.write(b"\n=== cleaned ===\n")
            f.write(cleaned.encode("utf-8", errors="replace"))
            f.write(b"\n\n=== raw (with ANSI) ===\n")
            f.write(raw)
    except OSError:
        pass


def main() -> None:
    stats: dict = {}
    try:
        raw = capture(stats)
    except Exception as e:
        print(json.dumps({"error": f"capture failed: {e}", "stats": stats}))
        return
    cleaned = ANSI_RE.sub(b"", raw).decode("utf-8", errors="replace")
    stats["total_bytes"] = len(raw)
    stats["cleaned_len"] = len(cleaned)

    # TUI output tends to eat spaces between words, so match flexibly.
    result: dict = {}
    session = find_section(cleaned, r"Current\s*session")
    week_all = find_section(cleaned, r"Current\s*week\s*\(all\s*models?\)")
    week_sonnet = find_section(cleaned, r"Current\s*week\s*\(Sonnet\s*only\)")
    if session:
        result["session"] = session
    if week_all:
        result["week_all"] = week_all
    if week_sonnet:
        result["week_sonnet"] = week_sonnet

    cost_m = re.search(r"Total cost:\s*\$([\d.]+)", cleaned)
    if cost_m:
        result["session_cost_usd"] = float(cost_m.group(1))

    if not result:
        # Dump everything to a debug file so we can diagnose without another
        # build cycle, and surface a short preview + stats in the error JSON.
        write_debug(raw, cleaned, stats)
        flat = re.sub(r"\s+", " ", cleaned).strip()
        head = flat[:600]
        tail = flat[-600:] if len(flat) > 600 else ""
        print(json.dumps({
            "error": "no usage sections found",
            "preview": f"HEAD: {head}  …  TAIL: {tail}",
            "stats": stats,
            "debug_log": DEBUG_LOG,
        }))
        return
    print(json.dumps(result))


if __name__ == "__main__":
    main()
