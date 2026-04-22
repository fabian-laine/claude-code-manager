/**
 * Short, discrete notification chime played when a background project
 * finishes its turn. Uses the Web Audio API so we don't need to ship an
 * audio file — two quick sine tones, ~180ms total.
 */
let ctx: AudioContext | null = null;

function getCtx(): AudioContext | null {
  if (typeof window === "undefined") return null;
  if (ctx && ctx.state !== "closed") return ctx;
  try {
    const AC: typeof AudioContext =
      window.AudioContext || (window as any).webkitAudioContext;
    ctx = new AC();
    return ctx;
  } catch {
    return null;
  }
}

function beep(freq: number, startDelay: number, durationMs: number): void {
  const c = getCtx();
  if (!c) return;
  const t0 = c.currentTime + startDelay;
  const osc = c.createOscillator();
  const gain = c.createGain();
  osc.type = "sine";
  osc.frequency.value = freq;
  // Quick attack + decay so it doesn't click.
  gain.gain.setValueAtTime(0, t0);
  gain.gain.linearRampToValueAtTime(0.18, t0 + 0.01);
  gain.gain.exponentialRampToValueAtTime(0.001, t0 + durationMs / 1000);
  osc.connect(gain).connect(c.destination);
  osc.start(t0);
  osc.stop(t0 + durationMs / 1000);
}

export function playFinishChime(): void {
  // Two rising tones — short, pleasant, hard to miss but not alarming.
  beep(660, 0, 90);
  beep(880, 0.09, 120);
}
