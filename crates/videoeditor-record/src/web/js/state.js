// Shared app state + tiny DOM/event helpers. Imports nothing — every other
// module leans on this one, so it must stay dependency-free.

// Modules talk through events, not imports of each other:
//   'clip-selected'  main      → takes (reload rail), review (reset card)
//   'record-start'   record    → takes (pause audition), review (reset card)
//   'take-recorded'  record    → review (detail: the recorded Blob)
//   'takes-changed'  review    → takes (a new take was archived — reload)
//   'clip-updated'   review/takes → main (re-select: refresh header + list)
export const bus = new EventTarget();

export const $ = (id) => document.getElementById(id);
export const fmt = (s) => s.toFixed(2);
export const pad3 = (n) => String(n).padStart(3, '0');

export function el(tag, cls, text) {
  const e = document.createElement(tag);
  e.className = cls;
  e.textContent = text;
  return e;
}

export const status = (msg) => ($('status').textContent = msg);

// idle | countdown | recording | review | uploading — mirrored onto
// <body class> so CSS drives what each mode shows (script size, review
// card, dimmed rails, countdown overlay, pulsing record button).
let state = 'idle';
export const getState = () => state;
export function setState(s) {
  state = s;
  document.body.className = s;
}

let episode = null;
let current = 0;
export const getEpisode = () => episode;
export const setEpisode = (e) => (episode = e);
export const getCurrent = () => current;
export const setCurrent = (i) => (current = i);
export const currentClip = () => episode.clips[current];
