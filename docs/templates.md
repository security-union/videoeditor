# Organizing templates: per-video, per-creator, per-machine

Scene templates don't have to live in the engine. Resolution is layered —
**most specific wins** — so you organize templates however fits your work,
and each video can look like nothing else you've made:

| Layer | Lives in | Use it for |
|---|---|---|
| 1. episode | `<episode>/templates/scenes/` | scenes unique to ONE video |
| 2. packs | any directory, declared in frontmatter `packs:` | your channel's visual identity, shared sets |
| 3. machine | `$VIDEOEDITOR_PACK_PATH` (colon-separated) | packs every project on this machine sees |
| 4. built-ins | ships with the binary | title-card, code-meme, duel-table, scoreboard |

A file named like a built-in **overrides** it (ship your own `title-card.html`
and every scene using `template=title-card` gets your version). A declared
pack that doesn't exist is a hard error. `videoeditor pack list <episode>`
prints the layers and the exact file every scene resolves to; renders log the
source whenever a template doesn't come from the built-ins.

## A channel laid out this way

```
my-channel/
├── brand-pack/                    # your identity — make it its own git repo
│   ├── templates/CLAUDE.md        # ← the authoring contract Claude follows
│   └── templates/scenes/
│       ├── _lib/  _vendor/        # vendored scene runtime (pack renders standalone)
│       ├── title-card.html        # overrides the built-in title card
│       └── neon-stat.html         # your custom scene
├── ep001-rust-vs-go/
│   ├── script.md                  # packs: ../brand-pack
│   └── templates/scenes/
│       └── boss-fight.html        # exists ONLY in this video
└── ep002-json-parsers/
    └── script.md                  # packs: ../brand-pack  (same identity, zero copying)
```

`ep001` resolves `boss-fight` from itself, `title-card`/`neon-stat` from
`brand-pack`, and anything else from the built-ins. Two creators exchange
looks by cloning each other's pack and adding one `packs:` line.

## Getting templates in

```bash
# a shared pack (creator identity)
videoeditor pack init brand-pack

# unique templates for one video — an episode's templates/ is just the
# most specific pack layer, so the same command works on the episode:
videoeditor pack init ep001-rust-vs-go
```

Both scaffold `templates/scenes/` with an example template, the vendored
`_lib`/`_vendor` scene runtime (templates reference it relatively, so it must
sit next to them), a README, and the authoring contract at
`templates/CLAUDE.md` (nested, so it never collides with an episode's own
CLAUDE.md).

Then declare shared packs in the episode's `script.md` frontmatter:

```markdown
---
title: Rust vs Go
packs: ../brand-pack, ../borrowed-lower-thirds
---

[SCENE: hook | template=boss-fight duration=2.5]
[DATA: title="ROUND ONE" title_at=300]
```

## Authoring templates — don't do it by hand

A template is one HTML file, a pure function of `(data, t)`: the engine
injects the merged `[DATA:]` map, calls `renderScene(t)` per frame, and
screenshots. No CSS animations, no timers — every pixel derives from the
input state, which is what makes renders deterministic.

**You should not hand-roll that.** `pack init` writes `templates/CLAUDE.md`,
which makes [Claude Code](https://claude.com/claude-code) the pack's template
engineer: open Claude Code in the pack, describe the look and the beats
("CRT terminal, green phosphor, the score slams in when the voice says
'benchmark'"), and Claude writes the template, renders one scene, reads the
extracted frames, and iterates with you until the frames look right.
