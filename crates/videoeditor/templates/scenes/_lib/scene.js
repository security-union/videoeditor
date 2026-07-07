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
