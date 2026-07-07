# videoeditor — the director's guide (for AI agents and humans)

You are directing a scripted short video: `script.md` in → vertical mp4 out.
This guide is embedded in the binary (`videoeditor guide`) so it is always
available, even far from the source repo.

## The pipeline

```
videoeditor new <dir> [--format meme-benchmark|blank]   scaffold (renders as-is)
videoeditor parse <dir>        resolved plan as JSON (scene starts, clips)
videoeditor tts <dir>          narration via ElevenLabs (needs ELEVENLABS_API_KEY)
videoeditor render <dir> [--scene name]      headless-Chrome frames → scene mp4s
videoeditor assemble <dir>     concat + narration@offsets + music → build/final.mp4
videoeditor build <dir>        tts + render + assemble
```

Discovery: `videoeditor templates [dir]` lists every scene template visible
(descriptions + data keys); `videoeditor preview [name]` renders contact
sheets of their demo data. `videoeditor pack list <dir>` shows which file
each scene resolves to (episode dir → `packs:` frontmatter →
`$VIDEOEDITOR_PACK_PATH` → built-ins; most specific wins).

## script.md in 20 lines

```markdown
---
title: My Short
voice_id: pNInz6obpgDQGcFmaJgB   # "Adam", an ElevenLabs public preset
packs: ../my-brand-pack           # optional shared template layers
music: assets/music/bed.mp3       # optional; skipped with a note if missing
---

[SCENE: hook | template=title-card duration=2.6]
[DATA: title="THE HOOK" title_at=1200]
[CLIP: hook | at=0.15]
Narration text until the next marker.

[SCENE: body | template=code-meme duration=7.0]
[DATA: code=assets/code/x.rs code_size=34 bench="μ: 1.2ms" bench_at=5.5]
[CLIP: body | at=0.2]
One idea per beat. The screen holds the digits; the voice tells the story.
```

Scene `duration` is authoritative. `at` = seconds from scene start. Paths are
episode-relative. `<!-- comments -->` and unknown `[MARKERS:]` are ignored.

## The director loop (do these IN ORDER, every time)

1. **Script** — casual, deadpan narration; at most ONE rounded number spoken
   per beat; every number shown on screen comes from a real, reproducible
   experiment (never invent benchmarks).
2. **`tts`** — then READ the ⚠ fit-check warnings. Narration overlap is a
   real defect (two voices at once). Recompute: scene duration = clip `at` +
   measured clip length + hold; re-place downstream `at`s. Re-run until clean.
3. **Congruence** — every visual cue (`*_at` keys, pointer cues, pops) lands
   when the narration says the thing. Cue time = clip `at` + clip length ×
   (char offset of the keyword ÷ clip text length).
4. **`render --scene X`** one scene at a time — then LOOK AT THE FRAMES in
   `<dir>/build/frames/<scene>/` (they are PNGs; read them). Heed ⚠ template
   warnings (e.g. clipped code). Layout bugs only show in frames.
5. **`assemble`**, then watch `build/final.mp4` end to end before calling it
   done. Anything readable must hold ≥1.5s; scoreboards ~4–5s.

## Templates

Prefer existing templates (see `videoeditor templates`). To create or
restyle: `videoeditor pack init <dir>` scaffolds a pack whose CLAUDE.md
teaches template authoring — the (data, t) pure-function contract, the
animation building blocks in `_lib/scene.js` (pop, enter, slam, shake,
pulse, count-up, word-pop, typewriter, Ken Burns, easings), self-diagnostics
(`sceneWarnings`), and the discovery block (`template-info`). Do not
hand-roll animation curves; compose the blocks.

## Env

- `ELEVENLABS_API_KEY` — required for `tts`/`analyze` only.
- `CHROME_BIN` — override the render browser.
- `VIDEOEDITOR_ROOT` — use a checkout's templates instead of the embedded ones.
- `VIDEOEDITOR_PACK_PATH` — colon-separated machine-wide template packs.
