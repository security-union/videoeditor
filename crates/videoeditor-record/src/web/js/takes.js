// The takes rail: every archived take for the current clip, newest first,
// with saved-analysis stats, audition, and approve.

import { $, fmt, el, bus, currentClip, status } from './state.js';

const player = new Audio();

bus.addEventListener('clip-selected', refresh);
bus.addEventListener('takes-changed', refresh);
bus.addEventListener('record-start', () => player.pause());

function refresh() {
  loadTakes().catch((e) => status('takes: ' + e.message));
}

// which take is expanded, keyed clip-id/file so re-renders (and coming
// back to a clip) restore it
let expanded = null;

async function loadTakes() {
  const c = currentClip();
  const takes = await (await fetch('/api/takes/' + c.id)).json();
  if (c !== currentClip()) return;   // switched clips mid-fetch
  $('takesEmpty').style.display = takes.length ? 'none' : 'block';
  const list = $('takesList');
  list.innerHTML = '';
  for (const t of takes) {
    const row = document.createElement('div');
    const key = `${c.id}/${t.file}`;
    row.className = 'take-row' + (t.approved ? ' approved' : '') + (key === expanded ? ' open' : '');
    const top = el('div', 'top', '');
    top.append(
      el('span', 'chev', '›'),
      el('span', 'file', t.file.replace('.mp3', '')),
      el('span', 'dur', fmt(t.duration) + 's'),
      playButton(`/audio/takes/${c.id}/${t.file}`),
    );
    if (t.approved) top.append(el('span', 'badge', '✓'));
    else top.append(approveButton(c, t.file));
    row.append(top);
    const meta = takeMeta(t.review);
    if (meta) row.append(el('div', 'meta', meta));
    row.append(detail(t));
    // the whole row is the disclosure control — one open at a time
    row.onclick = () => {
      const wasOpen = row.classList.contains('open');
      document.querySelectorAll('#takesList .take-row.open').forEach((r) => r.classList.remove('open'));
      expanded = wasOpen ? null : key;
      if (!wasOpen) row.classList.add('open');
    };
    list.append(row);
  }
}

// the expandable review: coaching notes + transcript from the saved analysis
function detail(t) {
  const d = el('div', 'take-detail', '');
  const inner = el('div', 'detail-inner', '');
  if (t.review) {
    const ul = document.createElement('ul');
    ul.className = 'coaching';
    for (const n of t.review.coaching || []) ul.append(el('li', '', n));
    inner.append(ul);
    if (t.review.transcript) inner.append(el('div', 'transcript', `heard: “${t.review.transcript}”`));
  } else {
    inner.append(el('div', 'no-review', 'no analysis saved for this take'));
  }
  d.append(inner);
  return d;
}

// one-line comparison stats from a take's saved coach report
function takeMeta(r) {
  if (!r) return '';
  const m = [];
  if (r.accuracy_pct != null) m.push(r.accuracy_pct.toFixed(0) + '% script');
  if (r.wps != null) m.push(r.wps.toFixed(1) + ' w/s');
  if (r.max_db != null) m.push('peak ' + r.max_db.toFixed(0) + 'dB');
  if (r.clipped) m.push('🔴 clipped');
  if (r.fits === false) m.push('⚠ long');
  if (r.pauses?.length) m.push('💀 ' + r.pauses.length + ' pause' + (r.pauses.length > 1 ? 's' : ''));
  return m.join(' · ');
}

function playButton(url) {
  const b = el('button', 'play', '▶');
  b.onclick = (e) => {
    e.stopPropagation();   // don't toggle the row's disclosure
    if (!player.paused && player.dataset.url === url) {
      player.pause();
      b.textContent = '▶';
      return;
    }
    document.querySelectorAll('#takesList .play').forEach((p) => (p.textContent = '▶'));
    player.src = url;
    player.dataset.url = url;
    player.play();
    b.textContent = '⏸';
    player.onended = () => (b.textContent = '▶');
  };
  return b;
}

function approveButton(c, file) {
  const b = el('button', 'approve', 'approve');
  b.onclick = async (e) => {
    e.stopPropagation();   // don't toggle the row's disclosure
    b.disabled = true;
    const res = await fetch(`/api/approve/${c.id}/${file}`, { method: 'POST' });
    if (!res.ok) {
      status('approve failed: ' + (await res.text()));
      b.disabled = false;
      return;
    }
    const info = await res.json();
    c.take_duration = info.duration;
    status(
      `approved ${file} → ${c.id}.mp3` +
      (info.fits ? ' ✓ fits' : ` ⚠ over the ${fmt(info.window)}s window`) +
      (info.warnings.length ? ` · ${info.warnings.length} timeline warning(s)` : ''),
    );
    bus.dispatchEvent(new Event('clip-updated'));
  };
  return b;
}
