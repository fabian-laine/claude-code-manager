/**
 * Minimal mic recorder that produces a 16-bit PCM WAV blob at the device's
 * native sample rate. The Rust backend resamples to 16 kHz before feeding
 * Whisper, so we don't need to resample here.
 */
export class MicRecorder {
  private stream: MediaStream | null = null;
  private ctx: AudioContext | null = null;
  private proc: ScriptProcessorNode | null = null;
  private source: MediaStreamAudioSourceNode | null = null;
  private chunks: Float32Array[] = [];
  private sampleRate = 0;
  private running = false;

  async start(): Promise<void> {
    if (this.running) return;
    this.stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    // @ts-ignore — webkitAudioContext fallback
    const AC: typeof AudioContext = window.AudioContext || (window as any).webkitAudioContext;
    this.ctx = new AC();
    this.sampleRate = this.ctx.sampleRate;
    this.source = this.ctx.createMediaStreamSource(this.stream);
    this.proc = this.ctx.createScriptProcessor(4096, 1, 1);
    this.proc.onaudioprocess = (e: AudioProcessingEvent) => {
      const input = e.inputBuffer.getChannelData(0);
      this.chunks.push(new Float32Array(input));
    };
    this.source.connect(this.proc);
    // Some browsers (Chrome) require the processor to be connected to the
    // destination for onaudioprocess to fire. Route to a muted gain node
    // so we don't actually hear ourselves.
    const sink = this.ctx.createGain();
    sink.gain.value = 0;
    this.proc.connect(sink);
    sink.connect(this.ctx.destination);
    this.running = true;
  }

  async stop(): Promise<Uint8Array> {
    if (!this.running) return new Uint8Array();
    this.running = false;
    try {
      this.proc?.disconnect();
      this.source?.disconnect();
      this.stream?.getTracks().forEach((t) => t.stop());
      await this.ctx?.close();
    } catch {
      /* ignore */
    }
    const total = this.chunks.reduce((a, c) => a + c.length, 0);
    const merged = new Float32Array(total);
    let off = 0;
    for (const c of this.chunks) {
      merged.set(c, off);
      off += c.length;
    }
    this.chunks = [];
    const rate = this.sampleRate;
    this.sampleRate = 0;
    return encodePcmWav(merged, rate);
  }

  get isRunning(): boolean {
    return this.running;
  }
}

function encodePcmWav(samples: Float32Array, sampleRate: number): Uint8Array {
  const buffer = new ArrayBuffer(44 + samples.length * 2);
  const view = new DataView(buffer);
  writeStr(view, 0, "RIFF");
  view.setUint32(4, 36 + samples.length * 2, true);
  writeStr(view, 8, "WAVE");
  writeStr(view, 12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  writeStr(view, 36, "data");
  view.setUint32(40, samples.length * 2, true);
  let off = 44;
  for (let i = 0; i < samples.length; i++) {
    const s = Math.max(-1, Math.min(1, samples[i]));
    view.setInt16(off, s < 0 ? s * 0x8000 : s * 0x7fff, true);
    off += 2;
  }
  return new Uint8Array(buffer);
}

function writeStr(view: DataView, off: number, s: string): void {
  for (let i = 0; i < s.length; i++) view.setUint8(off + i, s.charCodeAt(i));
}
