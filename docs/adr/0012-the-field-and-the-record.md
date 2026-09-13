# ADR 0012 — The Field and the Record

- **Status:** Accepted — supersedes the Chart's encoding (§ "the Record") and the
  speed gate on the event language noted in `README.md`, both decisions of the
  passes beneath this one.
- **Date:** 2026-09-13

## Context

Three presentation passes have already landed: the deep-field observatory, the
luminous field (an HDR frame with bloom), and the legible observatory. The audit
that opened this pass found the language coherent and the frame technically
sound, and found four things the app was not doing:

1. **The event language never ran.** `Effects` is gated on `speed <= 16.0`
   (`app/src/appstate.rs`), and the default speed is ×100 (`app/src/ui.rs`), so
   on a default run there are no impact flashes, no Wave pulses, no death
   embers and no tremor at all. The most dramatic thing the app draws was
   invisible unless the user happened to drag the speed slider down.
2. **The Arena had no depth.** Rocks, ship and bullets are flat material over a
   static backdrop; the starfield has three brightness layers and no parallax;
   nothing in the field is lit by anything the simulation does. Measured against
   the frame's own quality this was the weakest surface in the window.
3. **The Chart was plotting the least legible quantity available.** It drew
   mean Waves. Measured on the shipped binary, a Generation holds 1–3 distinct
   Wave values and ~60% of the Population sits on Wave 0 for the first sixty
   Generations, so the headline series reads 0.00–0.26 on a 0–1 axis while the
   shaped score beside it goes 1451 → 2848. The best panel in the sidebar was
   showing the flattest thing in the run.
4. **The Population was invisible.** One Agent is drawn; the other 99 are
   evaluated and dropped. `EpisodeOutcome` already carries per-member alive
   time, Wave, Fitness, novelty and the seven behaviour descriptors across the
   app boundary every frame, and none of it was read.

Measured spread of the descriptors that could carry a population instrument
(seed set {2026, 7, 4213110680}, 60 Generations, population 100):

```
gen 20 | alive p10/p50/p90   5.3  21.2  60.0 | coverage 0.050 0.175 0.375
gen 40 | alive p10/p50/p90   8.2  41.7  60.0 | coverage 0.075 0.250 0.550
gen 60 | alive p10/p50/p90   7.6  60.0  60.0 | coverage 0.075 0.325 0.650
```

Alive time and coverage spread by an order of magnitude; Waves do not. That is
what decided which quantities this pass makes visible.

## Decision

The Arena becomes a place with **depth**, and the sidebar becomes a **record** of
what the Population did. Both halves are the same move: show the state itself
rather than a number standing in for it.

**The field.**

- The sky gains **parallax**: the three star layers and a new near dust layer
  are displaced against the watched Ship's velocity by their own depth, so the
  field reads as a volume the Ship moves through. Zero when motion is off, and
  a pure function of the World's own state — no wall clock anywhere in it.
- The Arena's **tremor is depth-weighted**. A distant nebula should not shake
  like a rock three metres away; each layer takes the jolt in proportion to its
  depth, the subject plane taking it whole.
- The **event language runs at every speed**. The speed gate is replaced by a
  _windowed observation_: the step loop hands the cosmetic layer the last
  quarter-second of simulation before each frame rather than either every step
  or none. Bounded, deterministic, and independent of how fast the machine is.
- **Impact** becomes a shock front drawn at the Impact's own recorded `radius`,
  a light spill onto the rock around it, and sparks with velocity — and firing
  gets a muzzle light. `Impact.radius` was recorded by the simulation and read
  by nobody.
- The **trail** becomes a continuous ribbon with a width curve, unwrapped across
  the toroidal seam so a wrap is a ribbon that leaves one edge and arrives at
  the other rather than a streak across the field, and it survives any speed.

**The record.**

- The Chart panel becomes the **Record**: one instrument in two registers. The
  lower register is the run — the shaped Fitness series, best and mean, which is
  the only curve in this game that climbs, labelled as the breeding score
  (ADR 0003) with the cleared-Wave share beside it as Competence.

  **The upper register is the Generation**: every member of the Population as a
  point in (coverage, alive time), tinted by its real novelty, filling in as
  Episodes land, with the previous Generation kept behind it as a ghost. This is
  what `app/src/cohort.rs` was written for and never wired to.

- The **Network** panel stops encoding one scalar in three channels at once
  (width, alpha and colour all carried `|activation × weight|`), and labels its
  quantity for what it is: one-hop pre-activation contributions, `Σ w·a`.
- The hull gains the **Near Boundary**: a broken line joining the bearings that
  actually found something, at the range each one measured, with a dot on every
  sample. It is drawn as _open chains and never closed into a loop_, and a chord
  is drawn only between bearings that are neighbours in bearing order and both
  of which hit — a chord across a bearing that read nothing would be an edge the
  simulation never measured. With nothing found on any bearing it draws nothing
  at all, caption included.

  This is the shape the design settled into, not the shape it started as. The
  first version closed the polygon around all nine readings, which is honest
  arithmetic and a dishonest picture: a bearing that found nothing says only
  "not inside five hundred units", and putting it at the range limit drew a
  decagon of straight chords across the whole field asserting walls that were
  not there. The golden-frame test caught it (a cell off by 41 levels) and the
  rewrite followed.

**The interface tokens.** `theme` gains a `motion` module (durations and
easing curves), a **Record** colour family on its own hue so the run's history
is not the same ink as a live perception reading, and gains for the new
emitters. `TEXT_FAINT` is lifted to clear WCAG AA on the panel it is printed on
— it was about 3.5:1 at 8–10 pt, and `theme::tests` now pins every reading ink's
ratio.

## Considered Options

- **Adopting `egui` 0.36.2 as an embedded panel layer.** It is the only
  immediate-mode GUI whose pins match this workspace exactly (`wgpu = "30.0"`,
  `winit = "0.30.13"`) and it can render into the app's own `RenderPass`. Rejected:
  its MSRV is 1.95 against this workspace's declared 1.89, it ships its own glyph
  atlas and font stack beside the one the app already tunes for the
  text-never-blooms rule, and making it read as this instrument rather than as a
  foreign layer is a restyle larger than the panels it would replace. Recorded
  as a real option, not a foregone one: if the panel count ever triples, revisit.
- **`glyphon` 0.12.0** (wgpu 30, cosmic-text 0.19) to replace the hand-rolled
  atlas. Rejected: it produces the same swash coverage masks from the same
  version of cosmic-text, so the pixels are identical and the win is ~250 deleted
  lines against a working, test-pinned path. The measured text problem is cost,
  not quality, and that is fixed with a shape cache.
- **A `vello`/`femtovg` renderer replacement.** `vello` 0.10 pins wgpu 29.0.3;
  `femtovg` 0.27 does pin wgpu 30 but runs its own encoder into its own texture
  and would orphan the Painter, the HDR frame and the bloom chain. Rejected; its
  shader is kept as a reference for coverage AA.
- **A bloom-free / afterimage-style accumulation buffer.** Rejected: a persistent
  full-resolution target breaks the paused frame's pixel identity and the golden
  frame's "pure function of the World" property, for an effect the geometry
  path can produce without either.
- **A lineage tree or a Species census.** No parentage is recorded, and the
  Species count measures at 1 for 500 Generations at shipped tuning
  (`TUNING.md`). Both would be instruments drawing a straight line.

## Consequences

`Painter` gains one primitive, `luminous_gradient_polygon` — the additive
gradient fill that lets a falloff be light instead of a stack of flat bands; the
Wave pulse's four hand-banded strips and every ring-stacked glow are the reason
it exists.

The cosmetic layer's speed _switch_ in `App::advance` is gone. It is replaced by
an observation _window_: every watched step is offered to `Effects` and `Trail`,
which keep only the last quarter-second behind the simulation clock, and the
frame commits that window once. At ×1 a frame is one step and the record is
byte-for-byte what it was; at ×100 the frame shows the last quarter-second
instead of nothing. `Trail`'s window also widened from 0.8 s to 2.0 s, because a
trace three-quarters of a second long is shorter than the hull's own halo at the
speeds the Population actually flies — it was drawn every frame and could not be
seen.

`ui::Layout.chart` becomes `ui::Layout.record` and the panel's natural height
goes 120 → 240, because it now carries two registers where the others carry one.
The sidebar's shrink priority is unchanged: the Network still gives way first.

`app/src/cohort.rs` is adopted into the crate (it was on disk, uncompiled and
unreferenced) and its `Progress`/`Point` half is deleted: a second per-Generation
history beside `Run::history` was duplication, and the Chart reads the
simulation's own record. `Cohort` is the half that was new, and `Member` gains
the member's own `novelty`.

The window's frame time budget is unchanged in intent: the cosmetic work is
bounded by construction (fixed pool caps, one bounded observation window per
frame, ribbon vertices bounded by a fixed sample count), and the pass must not
move the measured render cost materially at 1440×900 or 2880×1800.
