# The field and the record

The fifth presentation pass. It deepens the Arena into a volume and turns the
sidebar's Chart into a record of what the Population actually did. The decision
is [ADR 0012](../adr/0012-the-field-and-the-record.md); this is the evidence
under it — what was measured before anything changed, what was built, where each
idea came from, and what it costs.

Captures: [`docs/visuals/2026-09-field-and-record/`](../visuals/2026-09-field-and-record/).
The `before/` frames come from a clean worktree at `ab988f1`, built release and
driven by the same probe with the same arguments; `native-*.png` are real screen
captures of the release binary.

---

## 1. The audit

Three passes had already landed (deep field → luminous field → observatory,
legible), so the audit's job was to find what the language was _not_ doing. Four
things, each with a measurement behind it.

### 1.1 The event language never ran

`App::advance` gates the cosmetic layer on `speed <= 16.0`
(`app/src/appstate.rs`), and `ui::Controls::default().speed` is `100.0`
(`app/src/ui.rs`). **On a default run the app drew no impact flashes, no Wave
pulses, no death embers and no tremor at all** — the most dramatic thing it
draws was invisible unless someone dragged the speed slider down. This is the
single largest defect this pass fixed.

### 1.2 The Chart was plotting the least legible quantity available

The Chart drew mean Waves. A throwaway probe (`sim/examples/tmp_cloud_probe.rs`,
deleted after the measurement) ran the shipped population at seeds
{2026, 7, 4213110680} for 60 Generations and printed the spread of every
quantity the panel could have plotted:

```
seed       2026 gen  20 | alive p10/p50/p90    5.3   21.2   60.0 | cov 0.050 0.175 0.375 | dxwaves 1 | clear 0.00 | stats best 3313 mean 1451 mw 0.000
seed       2026 gen  40 | alive p10/p50/p90    8.2   41.7   60.0 | cov 0.075 0.250 0.550 | dxwaves 2 | clear 0.08 | stats best 7264 mean 2309 mw 0.080
seed       2026 gen  60 | alive p10/p50/p90    7.6   60.0   60.0 | cov 0.075 0.325 0.650 | dxwaves 2 | clear 0.09 | stats best 7315 mean 2411 mw 0.090
seed          7 gen  60 | alive p10/p50/p90    9.3   60.0   60.0 | cov 0.075 0.225 0.375 | dxwaves 2 | clear 0.04 | stats best 7268 mean 2263 mw 0.040
seed 4213110680 gen  60 | alive p10/p50/p90    6.6   60.0  101.6 | cov 0.075 0.250 0.625 | dxwaves 3 | clear 0.24 | stats best 11866 mean 2848 mw 0.260
```

Alive time and coverage spread by an order of magnitude. Waves do not: a
Generation holds one to three distinct values, so the headline series reads
0.00–0.26 on a 0–1 axis while the shaped Fitness beside it climbs 1451 → 2848.
**The best panel in the sidebar was showing the flattest thing in the run.**
`GenerationStats.best`/`.mean` — the score that is actually optimised — were
recorded every Generation and plotted nowhere.

### 1.3 The Population was invisible

One Agent is drawn; the other 99 are evaluated on the worker pool and dropped.
`EpisodeOutcome` already crosses the app boundary every frame carrying per-member
alive time, Wave, Fitness, novelty and the seven behaviour descriptors, and the
app read one field of it (`.steps`, for the measured rate).

`app/src/cohort.rs` — a complete recorder for exactly this, with a ghost
generation and an axis helper — was on disk, **not declared in `lib.rs`, not
compiled, and referenced nowhere**. It was adopted (see §4).

### 1.4 The Arena had no depth

Rocks, hull and shots are flat material over a backdrop that never moves. The
starfield had three brightness layers and no parallax. The tremor displaced
every layer identically, so a jolt read as _contents_ jittering over a stationary
nebula rather than as the field being struck. An external read of the pre-change
frame put it plainly: "the asteroids read as floating cutout forms on a plane
rather than objects occupying a deep volume."

### 1.5 What the audit did _not_ find

Worth recording, because it bounds this pass:

- **No unbounded growth** anywhere in the cosmetic path. Every pool is capped
  (`Effects` echoes 24 / waves 3 / bursts 2, `Trail` 48 samples), the vertex
  buffers grow to a power of two and then stop, and the glyph atlas resets rather
  than overflowing. The per-frame heap traffic is real (~15 families of small
  `format!`/`String`) but flat.
- **No stale state across transitions.** Restart, New seed, Load, Evolve and
  generation change all reach the same reset, and both history holders also
  self-guard on time regression.
- **Text is not clipped.** `Painter::set_clip` applies to triangles only;
  `TextItem` carries no clip. Every "stays inside its box" guarantee in the
  panels is arithmetic. This is a standing hazard, not a defect introduced here;
  the Record panel is written to keep its strings inside its rect by
  measurement.

---

## 2. What was built

### The field

| Change                                                                                                                | Where                                       |
| --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| Parallax: four sky layers displaced against the Ship's velocity by their own depth, wrapped about the torus           | `app/src/scene.rs`                          |
| A new nearest layer — dust motes, drawn as light, dimmer than the dimmest star behind them                            | `app/src/scene.rs`                          |
| Depth-weighted tremor: near layers take nearly the whole jolt, the far layer nearly none, the subject plane all of it | `app/src/scene.rs`                          |
| Windowed observation: the cosmetic layer is handed the last 0.25 s of simulation before each frame, at every speed    | `app/src/appstate.rs`, `app/src/effects.rs` |
| Shock front at the Impact's own recorded `radius`, a light spill on the rock, sparks with velocity, a muzzle flash    | `app/src/effects.rs`                        |
| The trace as a mitred, tapered ribbon, samples stored unwrapped across the seam                                       | `app/src/observatory.rs`                    |
| The Near Boundary: a broken line joining the bearings that found something                                            | `app/src/scene.rs`                          |

The parallax is a pure function of `ship.vx/vy` — nothing is integrated and no
history is kept — so a Ship that stops puts the sky back exactly where it was,
and reduced motion is exactly zero. The observation window replaced a speed
_switch_ with a bounded _slice_: at ×1 a frame is one step and the behaviour is
byte-for-byte what it always was; at ×100 the frame shows the last quarter-second
of the Episode instead of nothing.

### The record

The Chart panel became the **Record** — one instrument in two registers
(`app/src/panels.rs`, `app/src/ui.rs`).

- **The run** (lower register): the shaped Fitness series, best _and_ mean,
  which is the only curve in this game that climbs, labelled on its face as the
  breeding score rather than as skill (ADR 0003), with the cleared-Wave share as
  an exact proportion of the Population.
- **The Generation** (upper register): one point per member in (coverage, alive
  time), tinted by that member's own novelty over the Generation's own range,
  filling in as Episodes land, with the previous Generation behind it as a ghost
  and the watched member ringed and labelled.

The Network panel stopped encoding one scalar in three channels at once. Width is
constant, **opacity carries magnitude along the band**, and the caption says what
the number is: `2 largest |w·a| terms per node — one hop into its pre-activation
sum`, with cyan positive and amber negative. When the topology outgrows the box
the panel draws more hidden nodes first and, when it still cannot draw everything
the ranking selected, says `N of M drawn` rather than dropping them quietly.

The interface picked up the controls a person needs to find: a **Motion** key
beside Pause and Rays, a standing key legend on the Controls title row, per-hover
hints (including _why_ Evolve is waiting), and `TEXT_FAINT` lifted from ~3.5:1 to
4.8:1 on the panel ground it is printed on.

---

## 3. Where the ideas came from

The research went down the awesome-list chain to real source in every case; links
are pinned where a commit was obtainable. Only the _sources actually used_ are
listed here.

| Technique                                                                             | Source                                                                                                                                                                                                 | Use                                                                                                                                                                                                                |
| ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Mitred ribbon joints (`corner_out = 2·pos1 − corner_in`, bevel past a limit)          | [godotengine/godot `line_builder.cpp`](https://github.com/godotengine/godot/blob/master/scene/2d/line_builder.cpp) (MIT, in-file header)                                                               | The trace's joint solve. Algorithm only, credited in the comment.                                                                                                                                                  |
| Constant-lifetime ribbon, one sample per step, global space, size-over-lifetime taper | [bevy_hanabi `vfx_render.wgsl` @ `3848132`](https://github.com/djeedai/bevy_hanabi/blob/3848132f0048f8a4eafba4d685c00804dc7cdbc4/src/render/vfx_render.wgsl) (MIT OR Apache-2.0)                       | The rules, not the code: `bevy_hanabi` pins wgpu 29.0.3 and cannot be a dependency here.                                                                                                                           |
| Chromatic/radial post-process on impact; identity early-out while nothing is live     | [bevy `effect_stack` @ `8d743eb`](https://github.com/bevyengine/bevy/blob/8d743eb7dc7fcff57a7d5744d0bbf89620b1b145/crates/bevy_post_process/src/effect_stack/lens_distortion.wesl) (MIT OR Apache-2.0) | **Considered and rejected for this pass** — it needs a persistent full-resolution target, which breaks the paused frame's pixel identity. Recorded because the decision should be visible, not because it shipped. |
| Constant-width links, magnitude in opacity                                            | [BertViz `head_view.js`](https://github.com/jessevig/bertviz/blob/master/bertviz/head_view.js) (Apache-2.0)                                                                                            | The Network panel's encoding rule. TF Playground rides width instead; the panel picks one and says so.                                                                                                             |
| Emptiness as a first-class value in a quality-diversity map                           | [pyribs `_grid_archive_heatmap.py` @ `7bd723c`](https://github.com/icaros-usc/pyribs/blob/7bd723cf9863d480c8f2fec5ba9aa36f3b68c083/ribs/visualize/_grid_archive_heatmap.py) (MIT)                      | The cloud draws only members whose Episode has landed — nothing is interpolated or placed at a default.                                                                                                            |
| Instrument ladders: a moving scale whose centre row carries the exact number          | [ArduPilot `AP_OSD_Screen.cpp`](https://github.com/ArduPilot/ardupilot/blob/master/libraries/AP_OSD/AP_OSD_Screen.cpp) — **GPL-3.0, studied only, no code copied**                                     | Design precedent for the bench strip, which already carried it.                                                                                                                                                    |
| Motion durations and easing curves                                                    | IBM Carbon `packages/motion/src/tokens.ts` (Apache-2.0, © IBM)                                                                                                                                         | `theme::motion` — the six durations and the four curves are Carbon's published values.                                                                                                                             |
| WCAG 2.2 contrast                                                                     | [W3C WCAG 2.2 SC 1.4.3](https://www.w3.org/TR/WCAG22/#contrast-minimum)                                                                                                                                | `TEXT_FAINT` was below 4.5:1 at the 8–10 pt it is used at; `theme::tests::the_reading_inks_clear_aa_contrast` now pins every reading ink.                                                                          |
| Okabe–Ito colour-blind-safe palette                                                   | [Okabe & Ito 2008](https://jfly.uni-koeln.de/color/) (values are facts; cited, swatches not copied)                                                                                                    | Checked the Record family against it when choosing a third hue.                                                                                                                                                    |

**Flagged and not used.** Lygia is under the Prosperity Public License 3.0.0 —
free for noncommercial use, _not_ an open-source licence — so nothing was taken
from it; its README badge claims nothing and the licence file is the authority.
A munrocket WGSL SDF gist linked from awesome-webgpu states no licence and was
not copied. Inigo Quilez's article pages returned 404 to this environment, so
his fBm/SDF pages could not be licence-checked and nothing was taken from them
either.

---

## 4. Dependencies, and why there are none

**No runtime dependency was added, and one dead module was deleted.**

The research checked the current release of every candidate against this
workspace's pins (wgpu 30.0.1, winit 0.30.13, cosmic-text 0.19.0, rust-version
1.89):

| Candidate                           | Latest  | Pins                                                                                                 | What happened                                                                                                                                                                                                                                                                                                                                       |
| ----------------------------------- | ------- | ---------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `egui` + `egui-wgpu` + `egui-winit` | 0.36.2  | wgpu 30.0, winit 0.30.13 — **an exact match**, and `Renderer::render` takes an existing `RenderPass` | **Rejected.** MSRV 1.95 against this workspace's declared 1.89; its own glyph atlas and font stack would sit beside the one already tuned for the text-never-blooms rule; and restyling it into this instrument is a larger job than the panels it would replace. Recorded as a live option in ADR 0012 — if the panel count ever triples, revisit. |
| `glyphon`                           | 0.12.0  | wgpu 30.0.0, cosmic-text 0.19 — an exact match                                                       | **Rejected.** It produces the same swash coverage masks from the same cosmic-text, so the pixels are identical; the win is deleted code, not better text, against a working path that three tests pin.                                                                                                                                              |
| `vello`                             | 0.10.0  | wgpu **29.0.3**                                                                                      | Rejected on the pin; unchanged from the prior pass's rejection.                                                                                                                                                                                                                                                                                     |
| `femtovg`                           | 0.27.0  | wgpu 30.0.0, MSRV 1.88                                                                               | Rejected as a renderer replacement — it runs its own encoder into its own texture and would orphan the Painter, the HDR frame and the bloom chain. Its WGSL is kept as the reference for coverage AA if hairlines ever need it.                                                                                                                     |
| `bevy_hanabi`, `bevy_light_2d`      | 0.19    | bevy 0.19 → wgpu **29.0.3**                                                                          | Rejected as dependencies; their algorithms were read and adapted.                                                                                                                                                                                                                                                                                   |
| `noise` (noise-rs)                  | current | none — CPU only                                                                                      | Not needed: the deep field is baked on the CPU with this project's own integer hash, and that is load-bearing for the golden frame.                                                                                                                                                                                                                 |

The one dependency-shaped change is a **deletion**: `app/src/cohort.rs`'s
`Progress`/`Point` half was removed. It was a second per-Generation history
beside `Run::history`, and the Record reads the simulation's own record. The
`Cohort` half — the part with no equivalent — was adopted into the crate, and
`Member` gained the member's own `novelty`.

## 5. Verification

### Checks

```
cargo fmt --all -- --check                                  clean
cargo clippy --workspace --all-targets -- -D warnings       clean
cargo test --workspace --release                            205 tests, 0 failed
cargo build --workspace --release                           ok
```

The app crate's unit tests went from 85 to 121 — thirty-six new tests, all
asserting behaviour rather than source text: the parallax is exactly zero with
motion off and wraps rather than jumping at the seam; the boundary's vertices are
the exact inverse of the ray readings and no chord crosses a bearing that found
nothing; a cohort with three of five members settled draws exactly three points
and no more; a dot brightens with its own novelty and keeps the Record hue;
`best` is never drawn without `mean`; the shot's share bar is the exact
`clearing_share` fraction of the plot; the ribbon stays continuous across a seam
and bounds itself at different observation rates; reduced motion records nothing
at any speed; every reading ink clears WCAG AA on the ground it is printed on;
every control — the new Motion key included — resolves its own centre at four
window sizes. Two of those tests found real defects rather than pinning known
behaviour: `boundary_joins` was matching the `Option` wrapper instead of the
sample, so it would have drawn chords across bearings that found nothing, and the
nested `arena_scope` clipped each sky layer to its own, less-shaken copy of the
field.

The golden frame (`app/tests/golden/arena-960x600.txt`) did **not** need
regenerating: at 16×10 cell averages with an 8-per-channel tolerance the new
sky and the new boundary land inside it. The test still catches a gross change —
it failed by 41 levels on cell 134 while the boundary was drawing a filled
polygon across the field, which is how that defect was caught.

### The simulation is untouched

```
$ neuroarena-headless --seed 2026 --generations 40            > after.txt
$ diff <(before.txt | grep -v '^#') <(after.txt | grep -v '^#')
SIMULATION IDENTICAL
$ neuroarena-headless --seed 2026 --generations 40 --workers 1 | diff - after-1-worker
WORKER-COUNT IDENTICAL
```

Byte-identical to the pre-pass baseline and independent of worker count. No
constant in `sim/` changed; no file in `sim/` was edited except deleting a
throwaway example.

### Render cost

Five paired runs of the release probe per size, interleaved before/after, same
seed, same step count, same machine:

|                | before      | after       | delta                  |
| -------------- | ----------- | ----------- | ---------------------- |
| 1440×900, p50  | 1.161 ms    | 1.339 ms    | **+0.178 ms (+15.3%)** |
| 2880×1800, p50 | 1.982 ms    | 2.160 ms    | **+0.178 ms (+9.0%)**  |
| 1440×900, p95  | 4.883 ms    | 4.372 ms    | −0.511 ms              |
| triangles      | 2895 / 3187 | 3390 / 3438 | +17% / +8%             |
| glyphs         | 873         | 1089        | +25%                   |

Against a 16.7 ms budget at 60 Hz the frame cost grows by about 1.1% of one
frame. The p95 figures are noisy run to run (the machine is shared) and show no
consistent regression. The extra glyphs are the Record's legends and axis labels.

Long-run behaviour, release binary, running at the default ×100: RSS sampled
every thirty seconds for six minutes wandered between 313.7 and 321.8 MB with no
trend, and was **exactly flat at 314.5 MB for three minutes with the run
paused** — six samples thirty seconds apart, not one byte moved. So the render
path and the frame loop do not grow at all, and what does grow is
per-Generation bookkeeping in the running simulation, which is
pre-existing: the app spawns a worker thread and a fresh `Generation` per
Generation, and `Run::history` keeps one 48-byte `GenerationStats` for ever
because the Record plots it. Nothing this pass added allocates per frame beyond
what already did.

Interaction in the live window: keystrokes reach the application (Space was sent
from outside and the Pause key came back reading `Resume`, lit), but synthetic
_clicks_ do not, so the Motion key and the hover states were verified through the
probe's rendered states and the unit tests rather than by pointing at them.

### Visual

Every claim below is from a capture in
[`docs/visuals/2026-09-field-and-record/`](../visuals/2026-09-field-and-record/),
read back after the fact rather than asserted from the code.

- **The event language now runs at the default ×100.** Five live screen captures
  of the running binary at default settings show an expanding ring with a
  white-hot core and radial arcs, a warm flash contacting a rock, and a glowing
  band along the arena's edges. Before this pass, none of that was drawn at the
  default speed at all.
- **The impact reads.** `after/impact.png` was read back as: a prominent
  expanding ring, a very bright core, strong warm spill on the rock — rated 8/10
  for readability. The pre-pass frame of the same moment was read back as "more
  like a glowing contact marker than a forceful asteroid impact".
- **The trace reads as a wake.** `after/trace.png`: a slender, gently curved
  ribbon, broader and brighter at the hull, tapering and fading out. It was
  measured, not eyeballed: the probe reports the lit triangles the ribbon
  contributes, and that count runs from 108 when the Episode has barely started
  to 246 when the Ship has been flying. A first attempt at a 0.8 s window was
  **invisible** — the whole trace sat inside the hull's own halo — which is why
  the window is 2.0 s.
- **The Record works.** `after/g150.png`: a wide cloud of members with the
  previous Generation visible behind it, a jagged rising `best` curve over a
  smooth `mean`, and the cleared share at 27%.
- **Reduced motion is real and visible.** `after/reduced-motion.png`: the trace
  contributes **zero** triangles, no echoes, no tremor, and the Motion key draws
  plain instead of lit.
- **The Network panel fills its box.** `after/dense.png`: sixteen hidden nodes
  drawn out of forty-eight, labelled `+32 more`, readable connections, no
  clipping.
- **The Near Boundary reads.** `after/trace.png` and the live capture both read
  back as "a broken circular outline made of cyan dots and short cyan line
  segments" around the hull, clearly visible. This took two attempts: the line
  width and dot radius were first set while the instrument still had a filled
  polygon carrying it, and when the fill was removed as dishonest the marks were
  left too quiet to see at all — the caption was visible and the instrument was
  not. Edge 1.2 → 2.0 units, dots r 2.0 → 3.0, and `theme::light::ENVELOPE`
  1.35 → 1.7 (still under the Ray overlay's 1.8, because it is the shape under
  the readings rather than another reading).

---

## 6. Limits, and what was not verified

- **The pointer path was not exercised by hand.** Screen capture and keystrokes
  work in this environment, but synthetic mouse clicks do not reach the
  application, so every hover, press and drag state is verified by the probe's
  rendered output and by unit tests. The Motion button draws in both states and
  its key is wired and tested; a person should confirm the pointer path.
- **Depth in a still frame is subtle by construction.** Parallax is a _motion_
  effect: a single frame displaced by velocity looks identical to one laid out
  that way. A read-back of the live frame said so honestly — "I do not see a
  distinct foreground dust-mote layer". The dust does read (32 motes, bright
  specks over the dark field) and the sky keeps its still across a pause, but the
  depth claim rests on motion, which no still can evidence. This is the weakest
  part of the visual case for the pass.
- **The trace is faint early in a run, and sometimes absent.** A Generation-1
  Pilot covers ~30 Arena units a second, so its trace is short; a still capture
  of one hovering — as `after/g150.png` happens to be — shows no trail at all. It
  becomes a comet's tail as the Population gets faster (`after/trace.png`), and
  the ribbon is drawn whenever the Ship has moved, but a single frame cannot
  promise it is always long.
- **`impact.png` shows few separated sparks.** The capture is taken six steps
  after first contact; the sparks are at their smallest. The shock front and the
  spill carry the moment; the sparks do not yet.
- **Every capture is drawn with the Ray overlay off**, which is the app's
  default; adding `rays` to the probe's arguments turns it on. The probe was
  edited identically on both sides of the comparison so this is true of the
  `before` frames too.
- **The mean Fitness curve sits low against `best`.** That is the data — mean is
  roughly a third of best on a shared linear axis — but a read-back called it
  low-contrast. It is honest and it is legible; it is not flattering.
- **Not every capture is a matched pair.** Seven states are captured on both
  sides with identical arguments (`live`, `dense`, `impact`, `min`, `retina`,
  `g150`, `wide`). `before/native-960x640.png` has no `after` counterpart and
  `after/trace.png` and `after/reduced-motion.png` have no `before` counterpart —
  the first is a screen capture at a window size the later run did not use, the
  last two are states the pre-change code could not produce.
- **Network bundling (R15) and the sensorium time-strip (R10)**, named in
  `observatory-next.md` as researched-but-unbuilt, are **still unbuilt**. This
  pass changed the Network's encoding and height, not its layout algorithm, and
  the Near Boundary is a new instrument rather than the time-strip.
- **A parallel research ledger was deliberately left alone.**
  `docs/research/2026-09-material-and-light.md` is an in-flight sibling artifact
  — all five of its topics are still `(pending — background research in
progress)` — and it is uncommitted work belonging to another session. It sits
  partly over this pass's ground (lit polygons, parallax depth, thin-stroke
  quality), and those questions were answered here from the scout reports and
  the SYNTHESIS docs instead. The file is byte-for-byte as it was found; if that
  pass lands, its answers should be read before this one's are extended.
