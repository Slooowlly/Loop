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

function noiseBuffer(c, dur) {
  const buf = c.createBuffer(1, Math.floor(c.sampleRate * dur), c.sampleRate);
  const d = buf.getChannelData(0);
  for (let i = 0; i < d.length; i += 1) d[i] = Math.random() * 2 - 1;
  return buf;
}

// Transição suave e aérea (ruído por lowpass que abre e fecha devagar) — sem "corte".
export function whoosh() {
  const c = ensure();
  if (!c) return;
  const t = c.currentTime;
  const dur = 0.7;
  const src = c.createBufferSource();
  src.buffer = noiseBuffer(c, dur);
  const hp = c.createBiquadFilter();
  hp.type = "highpass";
  hp.frequency.value = 130;
  const lp = c.createBiquadFilter();
  lp.type = "lowpass";
  lp.Q.value = 0.4;
  lp.frequency.setValueAtTime(240, t);
  lp.frequency.linearRampToValueAtTime(850, t + 0.28);
  lp.frequency.linearRampToValueAtTime(180, t + dur);
  const g = c.createGain();
  g.gain.setValueAtTime(0.0001, t);
  g.gain.linearRampToValueAtTime(0.22, t + 0.14);
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  src.connect(hp);
  hp.connect(lp);
  lp.connect(g);
  g.connect(master);
  src.start(t);
  src.stop(t + dur);
}

// Blip curto e suave ao passar pelas opções do menu.
export function hover() {
  const c = ensure();
  if (!c) return;
  const t = c.currentTime;
  const o = c.createOscillator();
  o.type = "sine";
  o.frequency.setValueAtTime(880, t);
  o.frequency.exponentialRampToValueAtTime(1280, t + 0.06);
  const g = c.createGain();
  g.gain.setValueAtTime(0.0001, t);
  g.gain.linearRampToValueAtTime(0.1, t + 0.01);
  g.gain.exponentialRampToValueAtTime(0.0001, t + 0.09);
  o.connect(g);
  g.connect(master);
  o.start(t);
  o.stop(t + 0.1);
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
  // One-shot: entra, segura alguns segundos e sai (não vira loop chato).
  const now = c.currentTime;
  g.gain.setValueAtTime(0.0001, now);
  g.gain.linearRampToValueAtTime(0.09, now + 2.5);
  g.gain.setValueAtTime(0.09, now + 6);
  g.gain.linearRampToValueAtTime(0.0001, now + 9.5);
  const lfo = c.createOscillator();
  lfo.frequency.value = 0.06;
  const lfoG = c.createGain();
  lfoG.gain.value = 220;
  lfo.connect(lfoG);
  lfoG.connect(lp.frequency);
  lfo.start();
  const stopAt = now + 9.7;
  oscs.forEach((o) => o.stop(stopAt));
  lfo.stop(stopAt);
  oscs[0].onended = () => {
    ambient = null;
  };
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
