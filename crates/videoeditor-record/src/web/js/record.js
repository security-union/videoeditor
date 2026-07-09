// Record flow: countdown → MediaRecorder → timer → hand the blob to review.

import { $, fmt, bus, getState, setState, currentClip, status } from './state.js';
import { stream } from './mic.js';

let recorder = null;
let chunks = [];
let recStart = 0;
let timerId = null;

export function toggleRecord() {
  const s = getState();
  if (s === 'recording') stopRecording();
  else if (s === 'idle' || s === 'review') beginCountdown();
}

export function beginCountdown() {
  setState('countdown');
  bus.dispatchEvent(new Event('record-start'));
  const cd = $('countdown');
  let n = 3;
  cd.textContent = n;
  const iv = setInterval(() => {
    n -= 1;
    if (n === 0) {
      clearInterval(iv);
      startRecording();
    } else cd.textContent = n;
  }, 700);
}

function startRecording() {
  chunks = [];
  const mime = MediaRecorder.isTypeSupported('audio/webm;codecs=opus') ? 'audio/webm;codecs=opus' : 'audio/mp4';
  recorder = new MediaRecorder(stream(), { mimeType: mime });
  recorder.ondataavailable = (e) => chunks.push(e.data);
  recorder.onstop = () => {
    setState('review');
    status('listen back — keep it or go again');
    bus.dispatchEvent(new CustomEvent('take-recorded', { detail: new Blob(chunks, { type: recorder.mimeType }) }));
  };
  recorder.start();
  setState('recording');
  status('recording… space to stop');
  recStart = performance.now();
  const c = currentClip();
  const limit = c.window * c.tempo;
  timerId = setInterval(() => {
    const s = (performance.now() - recStart) / 1000;
    $('timer').innerHTML = `${fmt(s)}<span class="lim"> / ${fmt(limit)}</span>`;
    $('timer').classList.toggle('over', s > limit);
  }, 50);
}

function stopRecording() {
  recorder.stop();
  clearInterval(timerId);
}

$('recBtn').onclick = toggleRecord;
