// SFX procedurais via Web Audio API — sem arquivos de áudio.
// whoosh (transições), exportSuccess (chime), e um pad ambiente suave.

let ctx = null;
let master = null;
let ambient = null;

const MASTER_VOL = 0.6;

function muted() {
  try {
    return localStorage.getItem("loop.muted") === "1";
  } catch {
    return false;
  }
}

// Cria/retoma o contexto (autoplay exige gesto do usuário para tocar).
function ensure() {
  if (!ctx) {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return null;
    ctx = new AC();
    master = ctx.createGain();
    master.gain.value = muted() ? 0 : MASTER_VOL;
    master.connect(ctx.destination);
  }
  if (ctx.state === "suspended") ctx.resume().catch(() => {});
  return ctx;
}

export function isMuted() {
  return muted();
}

export function setMuted(m) {
  try {
    localStorage.setItem("loop.muted", m ? "1" : "0");
  } catch {
    /* ignore */
  }
  if (master) master.gain.value = m ? 0 : MASTER_VOL;
}

// Sopro filtrado (ruído com bandpass varrendo a frequência).
export function whoosh() {
  const c = ensure();
  if (!c) return;
  const t = c.currentTime;
  const dur = 0.5;
  const buf = c.createBuffer(1, Math.floor(c.sampleRate * dur), c.sampleRate);
  const d = buf.getChannelData(0);
  for (let i = 0; i < d.length; i += 1) d[i] = Math.random() * 2 - 1;
  const src = c.createBufferSource();
  src.buffer = buf;
  const bp = c.createBiquadFilter();
  bp.type = "bandpass";
  bp.Q.value = 0.7;
  bp.frequency.setValueAtTime(280, t);
  bp.frequency.exponentialRampToValueAtTime(1800, t + dur * 0.5);
  bp.frequency.exponentialRampToValueAtTime(360, t + dur);
  const g = c.createGain();
  g.gain.setValueAtTime(0.0001, t);
  g.gain.linearRampToValueAtTime(0.5, t + 0.05);
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  src.connect(bp);
  bp.connect(g);
  g.connect(master);
  src.start(t);
  src.stop(t + dur);
}

// Chime ascendente de confirmação (ex.: dados exportados).
export function exportSuccess() {
  const c = ensure();
  if (!c) return;
  const t = c.currentTime;
  const notes = [523.25, 659.25, 783.99]; // C5, E5, G5
  notes.forEach((f, i) => {
    const o = c.createOscillator();
    o.type = "sine";
    o.frequency.value = f;
    const g = c.createGain();
    const s = t + i * 0.1;
    g.gain.setValueAtTime(0.0001, s);
    g.gain.linearRampToValueAtTime(0.35, s + 0.02);
    g.gain.exponentialRampToValueAtTime(0.0001, s + 0.5);
    o.connect(g);
    g.connect(master);
    o.start(s);
    o.stop(s + 0.55);
  });
}

// Pad ambiente suave (acorde grave + LFO lento no filtro).
export function startAmbient() {
  const c = ensure();
  if (!c || ambient) return;
  const g = c.createGain();
  g.gain.value = 0.0001;
  g.connect(master);
  const lp = c.createBiquadFilter();
  lp.type = "lowpass";
  lp.frequency.value = 500;
  lp.connect(g);
  const freqs = [110, 164.81, 220]; // A2, E3, A3
  const oscs = freqs.map((f, i) => {
    const o = c.createOscillator();
    o.type = i === 0 ? "sine" : "triangle";
    o.frequency.value = f;
    o.detune.value = (i - 1) * 5;
    o.connect(lp);
    o.start();
    return o;
  });
  g.gain.linearRampToValueAtTime(0.09, c.currentTime + 3);
  const lfo = c.createOscillator();
  lfo.frequency.value = 0.06;
  const lfoG = c.createGain();
  lfoG.gain.value = 220;
  lfo.connect(lfoG);
  lfoG.connect(lp.frequency);
  lfo.start();
  ambient = { g, oscs, lfo };
}

export function stopAmbient() {
  if (!ambient || !ctx) return;
  const now = ctx.currentTime;
  const { g, oscs, lfo } = ambient;
  try {
    g.gain.cancelScheduledValues(now);
    g.gain.setValueAtTime(g.gain.value, now);
    g.gain.linearRampToValueAtTime(0.0001, now + 0.8);
    oscs.forEach((o) => o.stop(now + 0.9));
    lfo.stop(now + 0.9);
  } catch {
    /* ignore */
  }
  ambient = null;
}
