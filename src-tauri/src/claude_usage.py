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


def capture() -> bytes:
    claude_bin = resolve_claude()
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
        # Drain the initial UI for a few seconds before sending input.
        drain(fd, 5.0)
        # Type "/usage" one char at a time so the autocomplete menu settles.
        for c in b"/usage":
            os.write(fd, bytes([c]))
            time.sleep(0.05)
        time.sleep(0.5)
        os.write(fd, b"\r")
        out = drain(fd, 8.0)
        # Send exit so claude cleans up its terminal state.
        try:
            os.write(fd, b"/exit\r")
        except OSError:
            pass
        return out
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


def main() -> None:
    try:
        raw = capture()
    except Exception as e:
        print(json.dumps({"error": f"capture failed: {e}"}))
        return
    cleaned = ANSI_RE.sub(b"", raw).decode("utf-8", errors="replace")

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
        # Keep a preview of the cleaned TUI text so we can adjust regexes
        # without another build cycle when Claude changes its layout.
        preview = cleaned[-1200:].replace("\n", " ").replace("\r", " ")
        preview = re.sub(r"\s+", " ", preview).strip()
        print(json.dumps({"error": "no usage sections found", "preview": preview}))
        return
    print(json.dumps(result))


if __name__ == "__main__":
    main()
