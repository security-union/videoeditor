// The review card: AI coach feedback + playback + keep/retake, one place.
// The take is already archived server-side the moment /api/review runs —
// keep just approves it as the clip's audio.

import { $, fmt, pad3, bus, getState, setState, currentClip, status } from './state.js';
import { beginCountdown } from './record.js';

let blob = null;
let reviewTakeN = null;   // archive number the review pass stored this take under

bus.addEventListener('clip-selected', reset);
bus.addEventListener('record-start', reset);
bus.addEventListener('take-recorded', (e) => {
  blob = e.detail;
  $('player').src = URL.createObjectURL(blob);
  requestReview(blob);
});

function reset() {
  blob = null;
  reviewTakeN = null;
  $('player').removeAttribute('src');
}

async function requestReview(b) {
  const c = currentClip();
  $('coachHdr').textContent = 'COACH · analyzing take…';
  $('coachNotes').innerHTML = '';
  $('transcript').textContent = '';
  try {
    const res = await fetch('/api/review/' + c.id, {
      method: 'POST',
      headers: { 'content-type': b.type },
      body: b,
    });
    if (!res.ok) throw new Error(await res.text());
    const r = await res.json();
    bus.dispatchEvent(new Event('takes-changed'));   // take + analysis archived — show it
    if (getState() !== 'review') return;             // user already moved on
    reviewTakeN = r.take;
    const acc = r.accuracy_pct != null ? ` · script <b>${r.accuracy_pct.toFixed(0)}%</b>` : '';
    const pace = r.wps != null ? ` · ${r.wps.toFixed(1)} w/s` : '';
    $('coachHdr').innerHTML =
      `COACH · take ${pad3(r.take)} · <b>${fmt(r.duration)}s</b> / ${fmt(r.window)}s window` +
      `${acc}${pace} · peak ${r.max_db.toFixed(1)} dB`;
    $('coachNotes').innerHTML = r.coaching.map((n) => `<li>${n}</li>`).join('');
    $('transcript').textContent = r.transcript ? `heard: “${r.transcript}”` : '';
  } catch (err) {
    $('coachHdr').textContent = `COACH · unavailable (${err.message})`;
  }
}

export async function keepTake() {
  if (getState() !== 'review' || !blob) return;
  setState('uploading');
  status('saving…');
  const c = currentClip();
  // review already archived this take → approve it in place; otherwise
  // (review still in flight or failed) upload the blob directly
  const res = reviewTakeN != null
    ? await fetch(`/api/approve/${c.id}/take_${pad3(reviewTakeN)}.mp3`, { method: 'POST' })
    : await fetch('/api/take/' + c.id, {
        method: 'POST',
        headers: { 'content-type': blob.type },
        body: blob,
      });
  if (!res.ok) {
    status('save failed: ' + (await res.text()));
    setState('review');
    return;
  }
  const info = await res.json();
  c.take_duration = info.duration;
  status(
    `saved ${fmt(info.duration)}s` +
    (info.fits ? ' ✓ fits' : ` ⚠ over the ${fmt(info.window)}s window — retake or stretch the scene`) +
    (info.warnings.length ? ` · ${info.warnings.length} timeline warning(s), re-run tts fit-check` : ''),
  );
  setState('idle');
  bus.dispatchEvent(new Event('clip-updated'));
}

export function retake() {
  if (getState() === 'review') beginCountdown();
}

$('keepBtn').onclick = keepTake;
$('retakeBtn').onclick = retake;
