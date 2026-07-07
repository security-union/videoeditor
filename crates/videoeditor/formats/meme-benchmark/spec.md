# Format: meme-benchmark

The "X vs Y with receipts" dev short: a ~20-second tribal matchup backed by
real benchmark numbers, evolved into a receipts-driven mini-explainer.

> The channel-wide production rules (REAL experiments, on-screen code
> standards, review process, pacing) live in `videoeditor guide` and
> override anything here.

> HOOK (tribal matchup) → GOOD (approve-meme + receipts) → FANCY (flex-meme +
> receipts) → BAD (disgust-meme + rant + receipts) → MEME VERDICT OUTRO

## Timing budget (T = 18–25s, target 20)

| Beat | % of T | Template | Content |
|------|--------|----------|---------|
| title | 10% | `title-card` | "X vs Y" logos + flame + topic. Narration states the matchup, nothing else. |
| good | 32% | `code-meme` | Code + approving meme (cozy Pooh tier). 👉 pointer walks the code. Benchmark pops in the last ~15% of the beat. |
| fancy | 18% | `code-meme` | Visibly shorter code + tuxedo-tier meme. ONE claim ("cleaner"). Benchmark pop at end. |
| bad | 28% | `code-meme` | Code + disgusted meme. The rant beat: exactly two concrete insults ("extremely slow", "uses more memory"). Benchmark pop at end. |
| outro | 11% | `video-clip` | Licensed meme clip whose own audio IS the punchline + "PSA for ___ devs:" caption. Never explain the joke. |

## Hard rules

1. **Target 18–25s.** Watch-through rate is the ranking signal; 20s loops.
   A beat that earns its seconds (a readable scoreboard, a landed dunk) may
   push to ~27s — pace beats the stopwatch.
2. **Narration is continuous and deadpan** — it never acknowledges the memes.
3. **Every scene cut lands within ~0.2s of a narration phrase boundary.**
4. **One moving element per scene** (cinematic code typing, OR the 👉 pointer,
   OR a text pop). Typing is the default eye-candy for code panels
   (`typing=true type_from= type_to=`); finish typing before the benchmark
   pops — sequential motion reads clean, simultaneous motion reads busy.
5. **Receipts required**: real code on screen, benchmark with μ and σ. Slightly
   contestable numbers are a feature (Cunningham's Law drives comments).
6. **The loser gets the longest rant but the shortest mercy** — cut straight to the meme.
7. Benchmark text: `Execution time: / μ: <val> / σ: <val>` — meme typography
   (white, thick black outline), popped word-by-word over the meme panel.
   **The numbers must be readable: keep every benchmark on screen ≥1.5s**
   (pop it with the narration, then let the scene breathe before the cut —
   extend the scene tail rather than popping earlier). The receipts are the
   product; a receipt nobody can read is decoration.
8. Music: soft lofi bed ~-20dB, whole video, no ducking.
9. Pick fights with big communities. The attacked tribe defends (comments),
   the winning tribe shares. Both are engagement.

## Production notes

- AI narration (ElevenLabs) is the **preview**; the human host records the
  final take over the locked timing.
- `videoeditor analyze <ref.mp4>` produces the timing table for studying more virals.
