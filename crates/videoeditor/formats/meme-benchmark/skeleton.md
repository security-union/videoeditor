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
<!-- Format: meme-benchmark. Timing budget for 20s: 10/32/18/28/11%. -->

[SCENE: title | template=title-card duration=2.1]
[DATA: left=assets/logos/x.png right=assets/logos/y.png title="TOPIC" title_at=1200]
[CHUNK: hook | at=0.15]
X versus Y for TOPIC.

[SCENE: good | template=code-meme duration=6.4]
[DATA: code=assets/code/good.ext meme=assets/memes/approve.png pointer=true pointer_from=0.2 pointer_to=4.3]
[DATA: bench="Execution time:|μ: ???|σ: ???" bench_at=5.7]
[CHUNK: explain | at=0.2]
We SOLVE THE TASK by DOING THE THING, then aggregate the results.
[CHUNK: bench | at=4.9]
Here's the final benchmark.

[SCENE: fancy | template=code-meme duration=3.7]
[DATA: code=assets/code/fancy.ext meme=assets/memes/tuxedo.png]
[DATA: bench="Execution time:|μ: ???|σ: ???" bench_at=2.9]
[CHUNK: fancy | at=0.17]
Here we are using FANCY_LIB, and you can see that the code is a lot cleaner.

[SCENE: bad | template=code-meme duration=5.7]
[DATA: code=assets/code/bad.ext meme=assets/memes/disgust.png]
[DATA: bench="Execution time:|μ: ???|σ: ???" bench_at=4.2]
[CHUNK: rant | at=0.1]
For the Y version, we are using the SLOW_LIB library. It is extremely slow and it uses a lot more memory.

[SCENE: outro | template=video-clip duration=2.2]
[DATA: src=assets/clips/punchline.mp4 seek=0]
