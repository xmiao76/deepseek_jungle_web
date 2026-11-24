// Synthesized match sounds. The AudioContext is unlocked on the first gesture.
export type SoundKind = 'select' | 'move' | 'capture' | 'win' | 'lose' | 'draw' | 'click';

export class Sound {
  private ctx: AudioContext | null = null;
  muted = false;

  constructor(muted: boolean) {
    this.muted = muted;
  }

  unlock(): void {
    if (!this.ctx) {
      const Ctor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (Ctor) this.ctx = new Ctor();
    }
    if (this.ctx && this.ctx.state === 'suspended') void this.ctx.resume();
  }

  setMuted(muted: boolean): void {
    this.muted = muted;
  }

  private tone(freq: number, start: number, duration: number, type: OscillatorType = 'sine', peak = 0.14): void {
    const ctx = this.ctx;
    if (!ctx || this.muted) return;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = type;
    osc.frequency.value = freq;
    const t0 = ctx.currentTime + start;
    gain.gain.setValueAtTime(0.0001, t0);
    gain.gain.exponentialRampToValueAtTime(peak, t0 + 0.012);
    gain.gain.exponentialRampToValueAtTime(0.0001, t0 + duration);
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start(t0);
    osc.stop(t0 + duration + 0.05);
  }

  play(kind: SoundKind): void {
    if (this.muted) return;
    this.unlock();
    switch (kind) {
      case 'select':
        this.tone(680, 0, 0.07, 'triangle', 0.09);
        break;
      case 'move':
        this.tone(430, 0, 0.09, 'sine', 0.15);
        this.tone(575, 0.035, 0.07, 'sine', 0.08);
        break;
      case 'capture':
        this.tone(300, 0, 0.1, 'square', 0.08);
        this.tone(190, 0.05, 0.16, 'sawtooth', 0.09);
        break;
      case 'win':
        [523, 659, 784, 1047].forEach((f, i) => this.tone(f, i * 0.11, 0.24, 'triangle', 0.13));
        break;
      case 'lose':
        [392, 330, 262].forEach((f, i) => this.tone(f, i * 0.13, 0.28, 'sine', 0.11));
        break;
      case 'draw':
        [392, 330].forEach((f, i) => this.tone(f, i * 0.15, 0.3, 'sine', 0.1));
        break;
      case 'click':
        this.tone(520, 0, 0.04, 'triangle', 0.05);
        break;
    }
  }
}
