# videoeditor — the director's guide

You are directing a scripted short video: `script.md` in → vertical mp4 out.
This is the canonical rulebook, embedded in the binary — `videoeditor guide`
prints it anywhere, repo or not. Everything else (episode CLAUDE.md files,
READMEs) points here; if another doc disagrees with this one, this one wins.

## The pipeline

```
videoeditor new <dir> [--format meme-benchmark|blank]   scaffold (renders as-is)
videoeditor parse <dir>        resolved plan as JSON (scene starts, clips)
videoeditor tts <dir>          narration via ElevenLabs (needs ELEVENLABS_API_KEY)
videoeditor record <dir>       record narration YOURSELF: local web teleprompter + mic
videoeditor render <dir> [--scene name]      headless-Chrome frames → scene mp4s
videoeditor assemble <dir>     concat + narration@offsets + music → build/final.mp4
videoeditor build <dir>        tts + render + assemble
videoeditor analyze <ref.mp4>  transcript + scene-cut timing table of a reference
videoeditor image "prompt" -o <png>   AI still for assets/ (grok|imagen; --ref conditions on an image)
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

## The director loop (in order, every time)

1. **Script** per the craft rules below.
2. **`tts`** — or **`record`** to perform the narration yourself: it
   serves a teleprompter at localhost with a live level meter, writes kept
   takes to `audio/clips/<scene>__<clip>.mp3` (previous audio is archived
   in `audio/takes/`), and fit-checks each take against its scene window.
   Then READ the ⚠ fit-check warnings. Narration overlap is a
   real defect (two voices at once). Recompute: scene duration = clip `at` +
   measured clip length ÷ tempo + hold; re-place downstream `at`s. Re-run
   until clean. Recompute after EVERY voice or text change.
3. **Congruence** — every visual cue (`*_at` keys, pointer cues, pops) lands
   when the narration says the thing. Cue time = clip `at` + clip length ×
   (char offset of the keyword ÷ clip text length).
4. **`render --scene X`** one scene at a time — then LOOK AT THE FRAMES in
   `<dir>/build/frames/<scene>/` (they are PNGs; read them). Heed ⚠ template
   warnings (e.g. clipped code). Layout bugs only show in frames.
5. **`assemble`**, then watch `build/final.mp4` end to end before calling it
   done.

## Craft rules

**Receipts are REAL experiments — non-negotiable.** Every number on screen
comes from a reproducible experiment committed next to the episode (e.g.
`assets/bench/` with a one-command `run.sh`): best bench tool per ecosystem,
comparable discipline on all sides (≥30 samples), fully optimized builds
(`lto = true`, `codegen-units = 1` — optimization level can flip a result),
symmetric pure workloads, both axes (time AND memory, same metric both
sides), round AGAINST yourself, corroborate implausible numbers before
shipping, and explain surprises IN the clip. Keep a bench README with machine
specs, results, methodology, honesty notes. Slightly contestable numbers are
a feature (Cunningham's Law drives comments); wrong numbers are not.

**On-screen code** must compile at a freeze-frame. Never show `.unwrap()` —
display code uses `fn main() -> anyhow::Result<()>` + `?`. No filler
statements; end panels on a strong line (a gag comment beats boilerplate).
Display snippets are minimal but honest versions of the real bench code.

**Narration** is casual, conversational, deadpan — contractions fine, no
stiff constructions. Simple English; speak at most ONE rounded number per
beat and never read stat strings aloud — the screen holds the digits. Add
human stakes. Don't rush: let beats breathe; duration follows pace. Dunk the
loser explicitly — longest rant, shortest mercy. TTS: low stability reads
livelier; never `atempo` ≥ 1.2 (squeezed pauses sound robotic); write
flowing sentences — fragments read staccato.

**Visuals**: one moving element per beat — sequential motion reads clean.
Anything the viewer must read holds ≥1.5s; tables ~3s; scoreboards ~4–5s.
Design for a phone at arm's length (1080×1920): min ~34px code, ~60px
labels, ~90px headlines. Accumulating benchmark tables stay in the same
position across scenes so rows are comparable.

**Review before render**: run two independent passes over script + code +
bench — a methodology skeptic (compilability, false claims, roast-magnets)
and a shareability critic (what's on screen for muted viewers, the
screenshottable frame). Fix what they flag; the director's word wins.

**Timestamps for learners**: ship an `education.json` next to `script.md` —
one entry per explanation, the time it STARTS plus the exact question it
answers, phrased the way a learner would search it. Derive times from the
assembled timeline (`parse` scene starts + clip `at`s + keyword cues in the
measured audio) — real, never guessed; regenerate whenever timings change.

**Hygiene**: never commit renders (`build/`), generated narration (`audio/`),
or API keys. Committed media assets go through git-lfs.

## Templates

Prefer existing templates (`videoeditor templates`). For a custom look,
`videoeditor pack init <dir>` scaffolds a pack — works on an episode dir too
(its `templates/` is resolution layer 1). The pack's `templates/CLAUDE.md`
is the authoring contract: the (data, t) pure-function rules, the animation
building blocks in `_lib/scene.js` (pop, enter, slam, shake, pulse, count-up,
word-pop, typewriter, Ken Burns, easings), self-diagnostics (`sceneWarnings`),
and the discovery block (`template-info`). Do not hand-roll animation curves;
compose the blocks.

## Gotchas

- Scene indices prefix rendered files (`build/scenes/NN_name.mp4`) —
  inserting a scene shifts them; re-render or rename.
- Moving a clip between scenes renames its audio key
  (`<scene>__<clip>.mp3`) — `mv` the file to keep a good take.
- TTS takes vary run-to-run at low stability — re-roll a slow take
  (`tts <dir> --clip <name> --force`) before rewriting script text.
- Iterate visuals on ONE scene (`render --scene X`); full renders come last.

## Env

- `ELEVENLABS_API_KEY` — required for `tts`/`analyze` only.
- `XAI_API_KEY` — `image --provider grok` (the default; takes `--ref` images).
- `AI_STUDIO` (or `GEMINI_API_KEY`) — `image --provider imagen` (safety-filtered,
  no references; rejections say so — reroute those prompts to grok).
- `CHROME_BIN` — override the render browser.
- `VIDEOEDITOR_ROOT` — use a checkout's templates instead of the embedded ones.
- `VIDEOEDITOR_PACK_PATH` — colon-separated machine-wide template packs.
