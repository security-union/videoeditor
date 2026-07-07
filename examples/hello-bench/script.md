---
title: Rust vs Python — summing a million numbers
fps: 30
width: 1080
height: 1920
voice_id: pNInz6obpgDQGcFmaJgB
model_id: eleven_multilingual_v2
---

# hello-bench — the smallest end-to-end episode

<!--
The starter episode: four scene templates, SVG-only assets (nothing binary,
nothing licensed), one narration voice ("Adam", an ElevenLabs public preset).

⚠ The benchmark numbers below are ILLUSTRATIVE so the demo renders out of the
box. Before you publish anything real, follow PRODUCTION.md Rule 1: every
number on screen comes from a reproducible experiment you actually ran.

Render it:
  export ELEVENLABS_API_KEY=...   # https://elevenlabs.io
  videoeditor build examples/hello-bench
  open examples/hello-bench/build/final.mp4
-->

[SCENE: title | template=title-card duration=2.4]
[DATA: left=assets/logos/rust.svg right=assets/logos/python.svg left_label=Rust right_label=Python]
[DATA: title="SUM A MILLION NUMBERS" title_at=1200]
[CHUNK: hook | at=0.15]
Rust versus Python. Summing one million numbers.

[SCENE: good | template=code-meme duration=6.0]
[DATA: code=assets/code/sum.rs code_size=34 meme=assets/memes/approve.svg pointer=true pointer_from=0.4 pointer_to=3.2]
[DATA: bench="Execution time:|μ: 0.9ms" bench_at=4.4]
[CHUNK: explain | at=0.2]
In Rust, one iterator does the whole job, and the compiler turns it into a tight loop. Under a millisecond.

[SCENE: bad | template=code-meme duration=5.4]
[DATA: code=assets/code/sum.py code_size=34 meme=assets/memes/disgust.svg]
[DATA: bench="Execution time:|μ: 48ms" bench_at=3.8]
[CHUNK: rant | at=0.15]
Python runs the same loop fifty times slower, one boxed integer at a time.

[SCENE: rule | template=duel-table duration=5.0]
[DATA: title="WHEN TO USE WHICH" left=assets/logos/rust.svg right=assets/logos/python.svg left_label=Rust right_label=Python]
[DATA: rows="hot loops:glue code|number crunching:quick scripts|ship it:prototype it" row_pops="0.6:1.4,2.0:2.8,3.6:4.0"]
[CHUNK: rule | at=0.2]
Hot loops go to Rust. Glue code stays in Python. Know which one you're writing.

[SCENE: score | template=scoreboard duration=3.6]
[DATA: title="FINAL SCORE" rows="Rust 0.9ms|Python 48ms"]
[CHUNK: verdict | at=0.3]
Rust wins this one. It's not even close.
