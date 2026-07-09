// Mic capture + input picker + L/R level meters.

import { $ } from './state.js';

let mediaStream = null;
let audioCtx = null;
let analyserL = null;
let analyserR = null;

export const stream = () => mediaStream;

export async function initMic(deviceId) {
  if (mediaStream) mediaStream.getTracks().forEach((t) => t.stop());
  mediaStream = await navigator.mediaDevices.getUserMedia({
    audio: {
      deviceId: deviceId ? { exact: deviceId } : undefined,
      channelCount: 2,           // interface sends a stereo mix — take it as-is
      echoCancellation: false,   // raw voice: no call-style processing
      noiseSuppression: false,
      autoGainControl: false,
    },
  });
  audioCtx = audioCtx || new AudioContext();
  const splitter = audioCtx.createChannelSplitter(2);
  analyserL = audioCtx.createAnalyser();
  analyserR = audioCtx.createAnalyser();
  analyserL.fftSize = analyserR.fftSize = 1024;
  audioCtx.createMediaStreamSource(mediaStream).connect(splitter);
  splitter.connect(analyserL, 0);
  splitter.connect(analyserR, 1);
  meterLoop();
  await fillDevices();
}

async function fillDevices() {
  const devices = (await navigator.mediaDevices.enumerateDevices()).filter((d) => d.kind === 'audioinput');
  const sel = $('micSelect');
  const activeId = mediaStream?.getAudioTracks()[0]?.getSettings().deviceId;
  sel.innerHTML = '';
  devices.forEach((d) => {
    const o = document.createElement('option');
    o.value = d.deviceId;
    o.textContent = d.label || 'microphone';
    if (d.deviceId === activeId) o.selected = true;
    sel.appendChild(o);
  });
}

let meterRunning = false;
function meterLoop() {
  if (meterRunning) return;   // one loop; reads whichever analysers are current
  meterRunning = true;
  const buf = new Float32Array(1024);
  (function tick() {
    for (const [analyser, fill] of [[analyserL, $('meterL')], [analyserR, $('meterR')]]) {
      analyser.getFloatTimeDomainData(buf);
      let peak = 0;
      for (const v of buf) peak = Math.max(peak, Math.abs(v));
      fill.style.width = Math.min(100, peak * 130) + '%';
      fill.classList.toggle('hot', peak > 0.85);
    }
    requestAnimationFrame(tick);
  })();
}

$('micSelect').onchange = (e) => initMic(e.target.value);
