# PRODUCTION.md — how to make videos that don't get roasted

videoeditor renders whatever you script. These rules are the difference between
a clip that gets shared and a clip that gets dunked on in the comments. They
were distilled from producing a real dev-shorts channel; adapt them to yours.

## Rule 1 — Receipts are REAL experiments. Non-negotiable.

Every number shown on screen comes from a reproducible experiment committed
next to the episode (e.g. `assets/bench/` with a one-command `run.sh`):

- **Best tool per ecosystem**: criterion (Rust), mitata (Node), etc. Comparable
  discipline on both sides (≥30 samples, ~10s budget, sample stddev ÷N−1).
- **Fully optimized builds only**: `lto = true` + `codegen-units = 1` on both
  bench and release profiles. Optimization level can flip a result.
- **Pure workloads, symmetric across languages** — measure the thing the video
  names, nothing else; consume results via `black_box`/`do_not_optimize`.
- **Both axes: time AND memory**, same metric on both sides (peak RSS holding
  the parsed result).
- **Round AGAINST yourself** (1.9×, not 2.0×) and quote the most conservative
  measured number.
- Keep a bench README with machine specs, results table, methodology, and
  honesty notes (retired numbers, run variance, what flipped and why).
- **If a number looks implausible, corroborate before shipping** (staged
  probes, a zero-instrumentation `/usr/bin/time -l` kernel check).
- **Explain surprises IN the clip** — answer the top comment before it's posted.

Slightly contestable numbers are a feature (Cunningham's Law drives comments).
Wrong numbers are not.

## Rule 2 — On-screen code

- **Must compile at a freeze-frame.** Never `?` in a bare `fn main()`.
- **Never show `.unwrap()`.** Display code uses
  `fn main() -> anyhow::Result<()>` + `?`; bench code uses `.expect("msg")`.
- No filler statements. End panels on a strong line — a gag comment
  (`// no types. fingers crossed 🤞`) beats boilerplate.
- Display snippets are minimal but honest versions of the real bench code.

## Rule 3 — Episode structure (the meme-benchmark format)

HOOK (X vs Y title card) → CHALLENGE (glance the actual dataset — generate the
sample from the real file) → GOOD (approving meme, receipts) → FANCY (honest
trade-offs, not fake wins) → BAD (disgusted meme + dunk) → SCOREBOARD (held
~4–5s, own dunk line) → OUTRO clip that re-tribes the joke per episode.

## Rule 4 — Visual system

- Code on a floating 3D card with typing reveal; Ken Burns on meme panels.
- **Accumulating benchmark table** across scenes — comparable, same position,
  new row pops with the narration.
- Receipts must be readable: ≥1.5s hold minimum, tables ~3s, scoreboard ~5s.
- Memes: clean, logo-free panels; badges/labels composited by the engine
  (`badge=`, `label=`).

## Rule 5 — Narration & pacing

- **Casual, conversational, deadpan** — contractions fine, no stiff
  constructions.
- **Simple English; the screen holds the digits, the voice tells the story.**
  Speak at most ONE rounded number per beat; never read stat strings aloud.
  Add human stakes ("your pager goes off… and the server isn't even down").
- **Don't rush**: let beats breathe. Duration follows pace — exceeding a
  format's target length is fine when beats earn it.
- **Dunk the loser explicitly.** The loser gets the longest rant but the
  shortest mercy.
- TTS: low stability reads livelier; avoid `atempo` ≥ 1.2 (squeezed pauses =
  the robotic sound). Write flowing sentences — expressive TTS models perform
  them; fragments read staccato.
- **CONGRUENCE is law**: what you hear must match what you see. The hook
  narration mirrors the title-card text; the 👉 pointer lands on a line at the
  moment the narration names it; each topic gets its OWN screen — never talk
  concepts over stale numbers.
- Fit-check every clip after TTS: scene duration = clip `at` + raw/tempo +
  hold; visual cue time = at + (raw/tempo) × (char offset of the keyword ÷
  clip length). Recompute after EVERY voice or text change.

## Rule 6 — Review before render

Run two independent reviews of script + code + bench before rendering:
one **methodology skeptic** (compilability, false claims, roast-magnets —
fix them, keep deliberate Cunningham bait) and one **shareability critic**
(cleverness, what's on screen for muted viewers, the screencliptable frame).
Synthesize; the director's word wins.

## Rule 7 — Repo hygiene

- Media through git-lfs; bench outputs and `build/` stay untracked.
- Never commit produced videos or API keys.

## Rule 8 — Timestamps for learners (YouTube Education tab)

Ship an `education.json` next to `script.md`: one entry per explanation —
the time it STARTS plus the exact question it answers, written the way a
learner would search it. Derive timestamps from the assembled timeline
(`videoeditor parse` scene starts + clip `at` offsets + keyword cues located
in the measured TTS clips) — **real, never guessed**, regenerated whenever
timings change.

## Episode checklist

1. `videoeditor new my-short` (+ drop in your meme/logo assets)
2. Build the REAL experiment → `run.sh` → bench README (Rule 1)
3. Display code panels from the bench code (Rule 2)
4. `script.md`: structure per the format spec, data-driven receipts, casual
   narration
5. Two-perspective review (Rule 6) → apply
6. `videoeditor tts` → fit-check → adjust tempos/durations
7. `videoeditor render --scene X` per scene → **QA extracted frames** (layout
   bugs hide here: clipping, empty backdrops, premature pops) → `assemble`
8. `education.json` from the final timeline (Rule 8)
9. Watch `build/final.mp4` end to end before calling it done

## Engine gotchas

- Templates are pure functions of (data, t) — no CSS animations, no timers.
- HTML comments in script.md are stripped (never narrated).
- Scene indices prefix rendered files (`build/scenes/NN_name.mp4`) — inserting
  a scene shifts them; re-render or rename.
- Moving a clip between scenes renames its clip key
  (`<scene>__<clip>.mp3`) — `mv` the file to keep a good take.
- TTS takes vary run-to-run at low stability — re-roll a slow take before
  rewriting script text.
- Iterate visuals by rendering ONE scene (`--scene X`) and QA-ing extracted
  frames before any full render.
