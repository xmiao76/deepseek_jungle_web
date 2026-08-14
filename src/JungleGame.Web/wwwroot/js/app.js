// Synthesized sound effects via Web Audio — the desktop app has no audio assets, so the web
// edition creates them fresh. The AudioContext is unlocked on the first user gesture
// (browsers require one before audio can play). Also exposes the environment probe for the
// threading badge.
(() => {
    let ctx = null;
    let muted = false;

    const ensure = () => {
        if (!ctx) {
            const AC = window.AudioContext || window.webkitAudioContext;
            if (AC) {
                ctx = new AC();
            }
        }
        if (ctx && ctx.state === 'suspended') {
            ctx.resume();
        }
        return ctx;
    };

    // Unlock on the first gesture so later non-gesture sounds (AI moves, win fanfare) work.
    document.addEventListener('pointerdown', () => ensure(), { once: true });

    const tone = (freq, start, dur, type = 'sine', peak = 0.15) => {
        const c = ensure();
        if (!c || muted) {
            return;
        }
        const osc = c.createOscillator();
        const gain = c.createGain();
        osc.type = type;
        osc.frequency.value = freq;
        const t0 = c.currentTime + start;
        gain.gain.setValueAtTime(0.0001, t0);
        gain.gain.exponentialRampToValueAtTime(peak, t0 + 0.012);
        gain.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
        osc.connect(gain);
        gain.connect(c.destination);
        osc.start(t0);
        osc.stop(t0 + dur + 0.05);
    };

    window.jungleSound = {
        unlock: ensure,
        setMuted: (m) => { muted = m; },
        play: (kind) => {
            if (muted) {
                return;
            }
            switch (kind) {
                case 'select':
                    tone(660, 0, 0.08, 'triangle', 0.1);
                    break;
                case 'move':
                    tone(440, 0, 0.09, 'sine', 0.16);
                    tone(587, 0.035, 0.07, 'sine', 0.09);
                    break;
                case 'capture':
                    tone(330, 0, 0.1, 'square', 0.09);
                    tone(220, 0.06, 0.15, 'sawtooth', 0.11);
                    break;
                case 'win':
                    [523, 659, 784, 1047].forEach((f, i) => tone(f, i * 0.12, 0.24, 'triangle', 0.14));
                    break;
                case 'draw':
                    [392, 330].forEach((f, i) => tone(f, i * 0.16, 0.28, 'sine', 0.12));
                    break;
            }
        },
    };

    // Environment probes for the UI (threading badge).
    window.jungleEnv = {
        isCrossOriginIsolated: () => self.crossOriginIsolated === true,
    };
})();
