"""
Capture Claude Code's /usage output by spawning the CLI on a pseudo-tty,
typing /usage, then reading and parsing the TUI output.

This is intentionally fragile — /usage is a TUI-rendered view, not an API.
If Claude Code changes its output format we'll need to adjust regexes.

Outputs a single JSON line on stdout: the parsed usage, or {"error": "..."}
on failure.
"""
import json
import os
import pty
import re
import select
import signal
import sys
import time

ANSI_RE = re.compile(rb"\x1B\[[0-?]*[ -/]*[@-~]|\x1B\][^\x07]*\x07|\x1B[=><]")


def capture() -> bytes:
    pid, fd = pty.fork()
    if pid == 0:
        # child
        os.execvp("claude", ["claude"])
    try:
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
        print(json.dumps({"error": "no usage sections found"}))
        return
    print(json.dumps(result))


if __name__ == "__main__":
    main()
