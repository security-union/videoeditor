---
title: X vs Y — TOPIC
fps: 30
width: 1080
height: 1920
voice_id: pNInz6obpgDQGcFmaJgB
model_id: eleven_multilingual_v2
music: assets/music/bed.mp3
music_gain_db: -20
---

# X vs Y — TOPIC
<!-- Format: meme-benchmark. Timing budget for ~20s: 10/32/18/28/11%.
     This scaffold RENDERS AS-IS (placeholder SVGs + code are copied in) —
     replace assets and text as the episode takes shape. Drop a lofi bed at
     assets/music/bed.mp3 (missing music is skipped with a note).
     After `videoeditor tts`, heed the ⚠ fit-check warnings: scene duration
     = clip at + clip/tempo + hold. See `videoeditor guide`. -->

[SCENE: title | template=title-card duration=2.6]
[DATA: left=assets/logos/x.svg right=assets/logos/y.svg left_label=X right_label=Y]
[DATA: title="TOPIC" title_at=1200]
[CLIP: hook | at=0.15]
X versus Y for TOPIC.

[SCENE: good | template=code-meme duration=7.4]
[DATA: code=assets/code/good.rs code_size=34 meme=assets/memes/approve.svg pointer=true pointer_from=0.2 pointer_to=4.5]
[DATA: bench="Execution time:|μ: ???|σ: ???" bench_at=5.7]
[CLIP: explain | at=0.2]
We SOLVE THE TASK by DOING THE THING, then aggregate the results.

[SCENE: fancy | template=code-meme duration=6.8]
[DATA: code=assets/code/fancy.rs code_size=34 meme=assets/memes/tuxedo.svg]
[DATA: bench="Execution time:|μ: ???|σ: ???" bench_at=5.2]
[CLIP: fancy | at=0.17]
Here we are using FANCY_LIB, and the code is a lot cleaner.

[SCENE: bad | template=code-meme duration=8.0]
[DATA: code=assets/code/bad.py code_size=34 meme=assets/memes/disgust.svg]
[DATA: bench="Execution time:|μ: ???|σ: ???" bench_at=6.4]
[CLIP: rant | at=0.1]
The Y version uses SLOW_LIB. It is extremely slow and it uses more memory.

[SCENE: score | template=scoreboard duration=4.5]
[DATA: title="FINAL SCORE" rows="X ???|Y ???"]
[CLIP: verdict | at=0.3]
X wins. It's not even close.

<!-- Optional meme outro (the format signature) — needs a clip you supply:
[SCENE: outro | template=video-clip duration=2.2]
[DATA: src=assets/clips/punchline.mp4 seek=0 caption="PSA for Y devs:"]
-->
