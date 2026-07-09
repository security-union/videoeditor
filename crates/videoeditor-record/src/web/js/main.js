// Boot, clip list, selection, keyboard map.

import { $, fmt, bus, getState, setState, getEpisode, setEpisode, getCurrent, setCurrent, status } from './state.js';
import { initMic } from './mic.js';
import { toggleRecord } from './record.js';
import { keepTake, retake } from './review.js';
import './takes.js';

async function boot() {
  setEpisode(await (await fetch('/api/episode')).json());
  $('epTitle').textContent = getEpisode().title;
  select(0);
  await initMic();
}

function select(i) {
  setCurrent(i);
  setState('idle');
  const c = getEpisode().clips[i];
  $('which').textContent = `${c.scene} / ${c.clip}`;
  $('windowInfo').textContent =
    `window ${fmt(c.window)}s at tempo ${c.tempo}` +
    (c.take_duration != null ? ` · current take ${fmt(c.take_duration)}s` : ' · no take yet');
  $('promptText').textContent = c.text;
  $('timer').innerHTML = `0.00<span class="lim"> / ${fmt(c.window * c.tempo)}</span>`;
  $('timer').classList.remove('over');
  renderList();
  bus.dispatchEvent(new Event('clip-selected'));
}

function renderList() {
  const list = $('clipList');
  list.innerHTML = '';
  getEpisode().clips.forEach((c, i) => {
    const row = document.createElement('div');
    row.className = 'clip-row' + (i === getCurrent() ? ' active' : '');
    const fits = c.take_duration != null && c.take_duration / c.tempo <= c.window + 0.005;
    const dot = c.take_duration == null ? '' : (fits ? 'has-take' : 'too-long');
    row.innerHTML = `<div class="dot ${dot}"></div>
      <div class="name">${c.scene} / ${c.clip}</div>
      <div class="len">${c.take_duration != null ? fmt(c.take_duration) + 's' : '—'}</div>`;
    row.onclick = () => {
      if (getState() === 'idle' || getState() === 'review') select(i);
    };
    list.appendChild(row);
  });
}

// approve/keep changed which take is the clip — refresh header, list, rail
bus.addEventListener('clip-updated', () => select(getCurrent()));

document.addEventListener('keydown', (e) => {
  if (e.code === 'Space') {
    e.preventDefault();
    toggleRecord();
  } else if (e.key === 'Enter') keepTake();
  else if (e.key === 'r') retake();
  else if ((e.key === 'ArrowDown' || e.key === 'ArrowRight') && getState() === 'idle')
    select(Math.min(getCurrent() + 1, getEpisode().clips.length - 1));
  else if ((e.key === 'ArrowUp' || e.key === 'ArrowLeft') && getState() === 'idle')
    select(Math.max(getCurrent() - 1, 0));
});

boot().catch((e) => status('mic init failed: ' + e.message));
