// videoeditor scene runtime — templates are PURE FUNCTIONS of (data, t).
//
// Contract with the engine (src/chrome.rs):
//   1. The engine navigates to the bare <template>.html.
//   2. It injects the merged [DATA:] map via `__sceneInit(data)` — images
//      arrive as data: URIs, code files as `codeText`.
//   3. The template's `setupScene(d)` builds static DOM once; its
//      `renderScene(tMs)` applies the state for time t (pure, idempotent).
//   4. Per frame the engine calls `__sceneSeek(t)` then screenshots.
// No CSS animations, no timers — every pixel derives from (d, t).
//
// Manual debugging in a normal browser still works:
//   open <template>.html?d=<base64url JSON>&t=<ms>
window.SCENE = { t: 0, d: {} };

window.__sceneInit = (d) => {
  window.SCENE.d = d;
  if (typeof window.setupScene === 'function') window.setupScene(d);
  window.__sceneSeek(window.SCENE.t);
  return true;
};

window.__sceneSeek = (tMs) => {
  window.SCENE.t = tMs;
  if (typeof window.renderScene === 'function') window.renderScene(tMs);
  return true;
};

// Browser-debug fallback: hydrate from ?d=&t= query params when present.
window.addEventListener('DOMContentLoaded', () => {
  const p = new URLSearchParams(location.search);
  if (!p.get('d')) return;
  window.SCENE.t = parseFloat(p.get('t') || '0');
  const b64 = p.get('d').replace(/-/g, '+').replace(/_/g, '/');
  const padded = b64 + '='.repeat((4 - (b64.length % 4)) % 4);
  window.__sceneInit(JSON.parse(decodeURIComponent(escape(atob(padded)))));
});

// Pop-in helper: meme-style overshoot (scale 1.6 → 1.0 over 180ms) at `at` ms.
window.popAt = (at, t) => {
  if (t < at) return { visible: false, scale: 0, opacity: 0 };
  const p = Math.min(1, (t - at) / 180);
  const scale = 1.6 - 0.6 * (1 - Math.pow(1 - p, 2));
  return { visible: true, scale, opacity: Math.min(1, p * 2) };
};

window.applyPop = (el, at, t) => {
  const s = popAt(at, t);
  el.style.visibility = s.visible ? 'visible' : 'hidden';
  el.style.transform = `scale(${s.scale})`;
  el.style.opacity = s.opacity;
};

// Deterministic flicker in [-1, 1] — layered sines, seeded by t only.
window.flicker = (t, seed = 0) =>
  Math.sin(t * 0.023 + seed) * 0.5 +
  Math.sin(t * 0.041 + seed * 2.7) * 0.3 +
  Math.sin(t * 0.007 + seed * 5.1) * 0.2;

// ─────────────────────────────────────────────────────────────────────────
// ANIMATION LIBRARY — proven building blocks, all pure functions of t (ms).
// Compose these instead of hand-rolling curves. Every helper is
// deterministic: same t → same pixels, which is what keeps renders stable.
// ─────────────────────────────────────────────────────────────────────────

// Easing curves (p in 0..1)
window.ease = {
  linear: (p) => p,
  outCubic: (p) => 1 - Math.pow(1 - p, 3),
  inOutSine: (p) => 0.5 - 0.5 * Math.cos(Math.PI * p),
  outBack: (p) => { const c = 1.70158; return 1 + (c + 1) * Math.pow(p - 1, 3) + c * Math.pow(p - 1, 2); },
};

// Clamped, eased progress of a window starting at `at` lasting `dur` ms.
window.prog = (t, at, dur, easeFn = ease.outCubic) =>
  easeFn(Math.min(1, Math.max(0, (t - at) / dur)));

// enter: fade + slide-in from a direction. Good default for text blocks.
//   applyEnter(el, at, t, {dur=280, from='up'|'down'|'left'|'right', dist=46})
window.enterAt = (at, t, o = {}) => {
  const { dur = 280, from = 'up', dist = 46 } = o;
  if (t < at) return { visible: false, x: 0, y: 0, opacity: 0 };
  const p = prog(t, at, dur, ease.outCubic);
  const d = (1 - p) * dist;
  const [x, y] = { up: [0, d], down: [0, -d], left: [d, 0], right: [-d, 0] }[from];
  return { visible: true, x, y, opacity: p };
};
window.applyEnter = (el, at, t, o) => {
  const s = enterAt(at, t, o);
  el.style.visibility = s.visible ? 'visible' : 'hidden';
  el.style.transform = `translate(${s.x}px, ${s.y}px)`;
  el.style.opacity = s.opacity;
};

// slam: stamp lands — starts huge, slams to rest with a little rotation.
// The dunk move. applySlam(el, at, t, {dur=160, from=2.4, rot=-7})
window.slamAt = (at, t, o = {}) => {
  const { dur = 160, from = 2.4, rot = -7 } = o;
  if (t < at) return { visible: false, scale: from, rot: 0, opacity: 0 };
  const p = prog(t, at, dur, ease.outCubic);
  return { visible: true, scale: from - (from - 1) * p, rot: rot * p, opacity: Math.min(1, p * 3) };
};
window.applySlam = (el, at, t, o) => {
  const s = slamAt(at, t, o);
  el.style.visibility = s.visible ? 'visible' : 'hidden';
  el.style.transform = `scale(${s.scale}) rotate(${s.rot}deg)`;
  el.style.opacity = s.opacity;
};

// shake: decaying wobble AFTER something lands (pair with slam/pop).
// applyShake adds to an element already placed; returns px/deg offsets.
window.shakeAt = (at, t, o = {}) => {
  const { dur = 500, amp = 10, cycles = 5 } = o;
  if (t < at || t > at + dur) return { x: 0, rot: 0 };
  const p = (t - at) / dur;
  const decay = 1 - p;
  return {
    x: Math.sin(p * cycles * 2 * Math.PI) * amp * decay,
    rot: Math.sin(p * cycles * 2 * Math.PI + 1.3) * 1.6 * decay,
  };
};

// pulse: looping attention loop (subtle breathing scale). period/amp gentle
// by default — use sparingly, one pulsing element max per scene.
window.pulse = (t, o = {}) => {
  const { period = 900, amp = 0.05 } = o;
  return 1 + Math.sin((t / period) * 2 * Math.PI) * amp;
};

// countUp: a number counting to its final value. Screen holds the digits.
// applyCount(el, at, t, {to, dur=800, decimals=0, prefix='', suffix=''})
window.countAt = (at, t, o) => {
  const { to, dur = 800, decimals = 0 } = o;
  const p = prog(t, at, dur, ease.outCubic);
  return (to * p).toFixed(decimals);
};
window.applyCount = (el, at, t, o = {}) => {
  const { prefix = '', suffix = '' } = o;
  el.style.visibility = t < at ? 'hidden' : 'visible';
  el.textContent = prefix + countAt(at, t, o) + suffix;
};

// typeText: plain-text typewriter (for code panels use the code-meme
// template's per-char reveal). Renders the first N chars for time t.
window.typeText = (el, full, at, t, o = {}) => {
  const { cps = 28, cursor = '▌' } = o; // chars per second
  if (t < at) { el.textContent = ''; return false; }
  const n = Math.min(full.length, Math.floor(((t - at) / 1000) * cps));
  const done = n >= full.length;
  el.textContent = full.slice(0, n) + (done ? '' : cursor);
  return done;
};

// popWords: split once with splitWords(), then pop word-by-word (the
// benchmark-text move). popWords(el, at, t, {wordMs=210})
window.splitWords = (el, text) => {
  el.textContent = '';
  for (const line of String(text).split('|')) {
    const lineEl = document.createElement('div');
    for (const word of line.split(/\s+/).filter(Boolean)) {
      const w = document.createElement('span');
      w.className = 'word';
      w.style.display = 'inline-block';
      w.style.margin = '0 0.18em';
      w.textContent = word;
      lineEl.appendChild(w);
    }
    el.appendChild(lineEl);
  }
};
window.popWords = (el, at, t, o = {}) => {
  const { wordMs = 210 } = o;
  el.querySelectorAll('.word').forEach((w, i) => applyPop(w, at + i * wordMs, t));
};

// kenBurns: the slow push-in that keeps static panels alive.
// el.style.transform = kenBurns(t, durationMs, {zoom=1.06, driftX=0, driftY=8})
window.kenBurns = (t, dur, o = {}) => {
  const { zoom = 1.06, driftX = 0, driftY = 8 } = o;
  const p = Math.min(1, Math.max(0, t / dur));
  return `scale(${1 + (zoom - 1) * p}) translate(${driftX * p}px, ${driftY * p}px)`;
};
