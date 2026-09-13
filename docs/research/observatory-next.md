# NeuroArena — the observatory, legible

2026-09-13 · Apple M5 · macOS 26.4 (Darwin 25.4) · Rust 1.98.1 · native winit 0.30 / wgpu 30 / Metal

## Result and scope

The third presentation pass made the observatory hold up at every window size and every
moment: the Competence chart became a real instrument on the data the headless table
already prints, the bench instruments and the field's own readouts stay legible at the
minimum window, the controls show where the hands are, impacts land in the body, and the
frame's light keeps its hue while a paused window costs nothing. The simulation is
untouched; its outputs are byte-identical.

| Before (seed 2026, step 180)                  | After (same)                                    |
| --------------------------------------------- | ----------------------------------------------- |
| `../visuals/2026-09-audit/arena-1440x900.png` | `../visuals/2026-09-observatory-next/after.png` |

## 1 · Audit (evidence: `docs/visuals/2026-09-audit/`)

The full pipeline map and the defect table are in the audit record. The findings this pass
addressed, all observed against the running app or the offscreen captures:

| Priority | Finding                                                                                                                                                                                                                                                  | Evidence                                                       |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| High     | **The Chart is the largest low-information surface.** It plots only `mean_wave`; `median_wave`, `p90_wave`, `clearing_share` exist in `GenerationStats` and are read by nothing in the app. 150 Generations collapse to one spiky line with one y label. | `chart-150gens-1440x900.png`                                   |
| High     | **Arena-space text scales with the field.** At the minimum window (900×560) the rulers and the NEAREST HULL / V / T readouts render at ~4 px — the field's own numbers vanish exactly when the window is small.                                          | `arena-min-900x560.png`                                        |
| High     | **Threat Telemetry disappears silently below ~1142 px width** (compact strip drops section 02, leaving a 01 → 03 numbering gap).                                                                                                                         | `arena-1000x620.png`, `arena-min-900x560.png`                  |
| Medium   | **At wide aspects the strip detaches from the field** — instruments stretch across letterbox bands; motor tiles become 414 px progress bars.                                                                                                             | `arena-wide-2560x1080.png`                                     |
| Medium   | **Focus is the weakest interaction state** — drawn inside the control, it merges with an On/toggled fill; and digits open the seed field from anywhere, letters act mid-edit.                                                                            | `state-focus-1440x900.png`, `appstate.rs:583-604`              |
| Medium   | **No idle path.** A paused, settled frame still presents at every vsync (~1.3 % of a core).                                                                                                                                                              | `appstate.rs:930-934`                                          |
| Medium   | **Chart banding + newest label in the curve's own colour at the curve's corner.**                                                                                                                                                                        | `chart-150gens-1440x900.png` at 5×                             |
| Low      | Scattered literal font sizes; `scene::draw_arena` leaves transform+clip installed (a demonstrated foot-gun); the impact event reads small; no `ScaleFactorChanged` arm.                                                                                  | `panels.rs:161,168`, `scene.rs:244-246`, `appstate.rs:891-928` |

Baseline: `cargo test --workspace` 139 passed / 0 failed; release build clean; live app ran
2 m with no GPU error; sim seed 2026×12 and seed 7×8 byte-identical across worker counts.

## 2 · Research

The mandated lists (awesome-rust, awesome-webgpu, awesome-creative-coding) were worked as
in the prior pass, plus awesome-rust-gamedev, awesome-godot, awesome-charting,
awesome-data-visualization and others; every shortlisted entry was followed to a pinned
implementation file and license-checked. The full ledger is the third-pass shortlist of
2026-09-13; what this pass adopted from it:

| Technique                       | Source (pinned)                                                                                     | Decision                                                                      |
| ------------------------------- | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| 1-2-5 axis ticks                | plotters `numeric.rs::compute_f64_key_points` (MIT); d3-array `ticks.js` (ISC)                      | Adapted as a dozen-line generator in `panels.rs`, credited in its doc comment |
| Median→p90 band + direct labels | d3-shape `stack.js` (ISC); observable plot `cell.js` (ISC)                                          | Independently implemented with the project's own fill machinery               |
| Interaction state tokens        | egui `style.rs` @ `f2de65f` (MIT OR Apache-2.0)                                                     | Independently implemented in `appstate`/`ui`                                  |
| Trauma shake                    | Vlambeer/GDC trauma model; `bevy_trauma_shake` (Apache-2.0) decay/`trauma²` shape                   | Independently implemented in `effects.rs`                                     |
| Bloom tone-mapping              | Khronos **PBR Neutral** (Apache-2.0) via bevy v0.19.1 `tonemapping_shared.wgsl` (MIT OR Apache-2.0) | Adapted verbatim (minus the toe) into `shader.wgsl`; both notices kept        |
| Interleaved-gradient grain      | Jimenez SIGGRAPH 2014; filament `surface_light_indirect.fs` (Apache-2.0)                            | Three-line independent implementation                                         |

Rejected again, with reasons unchanged: vello and a full engine (ADR 0004), the Book of
Shaders (all rights reserved — verified), Shadertoy default licence, temporal
auto-exposure (breaks paused-frame identity), motion blur/DoF, sprite assets. A lineage
view was rejected on honesty: `sim` carries no ancestry data, and inventing it would be
fabrication; the species census idea was deferred as a fourth-pass candidate.

**Dependencies added: none.** No new crates; no new fonts or binary assets — the two OFL
faces are unchanged. B612 (OFL-1.1) was vetted and deliberately not adopted: a third face
in a small app reads as indecision.

## 3 · Direction

The two prior passes built the field; this pass is **"the observatory, legible"** — the
instrument DNA (cyan perception, amber energy, white numbers, no invented telemetry)
sharpened rather than repainted. The signature element (the Agent wearing its mind on the
hull) is untouched. What changed is the observatory's competence at its job: the Chart
tells the learning story, the bench reads at the minimum legal window, the frame reacts to
what it witnesses, and a held frame is pixel-identical and free.

## 4 · Implementation

### The Chart instrument — `app/src/panels.rs` (draw_chart ~322-500), `theme.rs`

- **Axis.** A 1-2-5 ladder (`tick_step`/`rungs`) lays 2-4 labelled gridlines with an
  explicit 0 baseline; labels carry the step's own precision (`0.1`, not `0.10`). The
  Scale maps `0..peak` and clamps, so the p90 tail past the crest stays in the housing.
- **The band.** The median→p90 range is drawn as a flat wash per Generation in
  `color::BAND` (deep cyan at wash strength, ≤ white — pinned by the extended
  `nothing_the_interface_prints_can_bloom` test). The mean stays the subject: 1.6 px
  `color::BEST` stroke, marker dot, LAMP glow. The counts are whole Waves per Generation,
  so a flat column states that rather than implying a slope that was never observed.
- **Banding fixed.** The area fill is one ramp over the absolute plot height —
  neighbouring columns agree exactly along shared edges (regression-tested), replacing the
  per-window-pair gradient spans that read as stripes.
- **Direct labels.** The newest mean value moved into the panel heading row, right-aligned
  in `color::TEXT` where it was `BEST`-on-`BEST` at the curve's corner. The legend names
  both marks. The empty state draws its axis and zero line under "Awaiting first
  Generation" instead of a dead framed box.
- **Typography tokenised** (audit L1): the HUD headline literals (28) became
  `font::HEADLINE`; the Network's 9/8 px literals became `font::MICRO`/`font::FINE`. No
  literal text sizes remain in `panels.rs`; JetBrains Mono unchanged.

### The bench and the field — `observatory.rs`, `instruments.rs`, `scene.rs`, `painter.rs`, `effects.rs`

- **Arena-space text holds its size.** `draw_tracking` routes rulers and readouts through
  `arena_text(fit, size)`: on-screen size = `clamp(size·fit, 9.5, size·2.0)`. At the
  minimum window the rulers read at 9.5 px instead of 4; at 2× they do not blow past 2×.
- **The strip measures the field.** `strip_rect` is built from the arena view rect (min
  width 460), so at 2560×1080 the strip's edges coincide with the field's and the motor
  tiles are ~120 px, not 414.
- **Compact strip reflows instead of dropping.** Below 740 px, section 02 becomes
  "02 / THREAT" with three half-height two-line rows — all three sections (01/02/03) are
  present at every legal window; the numbering gap is gone.
- **Trauma shake.** Impact adds 0.45, death 0.85; decay 0.8/s; displacement =
  6 px · trauma² · coherent hash offset (cosmetic RNG, quantised to 3 steps), applied to
  the Arena transform only — never the strip or panels. The `m` toggle silences it with
  trails and echoes; it resets on episode change like the other effects. A paused frame
  keeps the offset it was given and stays pixel-identical. The displacement is App-owned
  state: `appstate` reads `Effects::shake()` after the effects pass observes and carries
  it on the view the next frame draws with — no hidden channel between the passes, and
  the rendering is byte-identical to the interim version (verified by pixel diff).
- **`Painter::arena_scope`.** Sets transform+clip for the arena and restores both whatever
  the body does — the audit's demonstrated foot-gun is unrepresentable now.

### The frame loop and the hands — `appstate.rs`, `ui.rs`

- **Idle path.** `wants_redraw()` = running, or a banner is fading, or an event is
  pending. Paused-and-quiet, the loop requests nothing and presents nothing: measured
  **0.0 % CPU** (top, 3 samples) versus ~1.0–1.6 % while running. Any event returns to
  continuous redraw in the same turn. Grain stays pixel-pure (no frame index anywhere).
- **Focus drawn outside.** `draw_focus_ring` strokes `ui::focus_ring(face)` — offset 2.0,
  weight 1.5, straddling 2–3.5 px outside the control — instead of `inset(2)` inside,
  where it merged with a toggled control's accent border. Hover, focus and pressed are now
  visibly distinct on On-state controls.
- **Hotkey discipline.** Digits open the seed field only when it is focused or hovered;
  `m/r/s/l/e` return early while the field is editing (Escape still leaves it). A stray
  keystroke can no longer open edit mode or Save mid-run invisibly.
- **`ScaleFactorChanged`** now reconfigures the surface and requests a frame instead of
  falling through.

### The light resolves — `shader.wgsl`

- **Tone-mapped bloom.** `fs_composite` resolves `scene + resolve_bloom(bloom·intensity)`.
  PBR Neutral was chosen over ACES-Fitted on measurement: it preserves hue exactly
  (0.0° across amplitudes 0.02–7.0) where ACES shifts 1-3° and cuts the bloom range hard
  (luminance retention 0.65–0.69). The fit's low-end toe is deliberately omitted — applied
  to the bloom term it swallowed the halo tail. Interface colours (≤ white) never touch
  the operator; ADR 0011's value separation stands. `BLOOM_INTENSITY` 0.215 is unchanged —
  measured max change 6/255, PSNR 59 dB against the previous resolve.
- **IGN grain.** `hash21` replaced by interleaved gradient noise — same call shape, same
  arena-rect mask, same off-switch, no temporal term, so the golden frame and the paused
  frame stay pure functions of pixel coordinates.

## 5 · Verification

### Commands

```sh
cargo fmt --all -- --check                              # clean
cargo clippy --workspace --all-targets -- -D warnings   # clean
cargo test --workspace                                  # 166 passed, 0 failed (was 139)
cargo build --workspace --release                       # clean
```

No pre-existing failures. Two clippy errors in the new chart code (negated partial-ord
compare, constant `chunks_exact`) were fixed, not allowed; the workspace lints remain at
`-D warnings`.

### Regression coverage added (27 tests)

| Area        | Tests pin                                                                                                                                                                                                                                                                                                                                                                            |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Chart       | 1-2-5 step choice (0.34 → 0.1; 2-4 gridlines; explicit 0); band geometry median→p90 with clamping; fill shared-edge equality (banding); empty-state axis + message; newest readout placement; band ink presence/absence                                                                                                                                                              |
| Theme       | new `BAND` colour ≤ white                                                                                                                                                                                                                                                                                                                                                            |
| Frame loop  | `wants_redraw` state matrix; Space earns exactly one lamp frame then silence; focus-ring geometry outside the face; hotkey truth table (digits gated, letters deferred while editing); ScaleFactorChanged re-layout                                                                                                                                                                  |
| Bench/field | arena-text floor at fit 0.5/1/2 + 2× device; readings stay on their subjects; strip measures the field at wide and minimum windows; compact strip reflows rather than dropping 02; trauma rise/saturate/decay/bound, reduced-motion stillness, held-frame offset, reset; `arena_scope` restores painter state; tremor moves the field and nothing else; seam copies hold under shake |
| Light       | gain-5 halo keeps its hue (pre-change shader fails this test at ~60° vs 43°); IGN deterministic and low-discrepancy vs the old white-noise hash; printed white stays ≤40/255 near a light (independent restatement of the ADR 0011 property)                                                                                                                                         |

The golden frame still passes unchanged and was **not** regenerated: the 16×10 cell grid
stays within tolerance (the composite's resolve is measured at 6/255 max, and the Chart is
not part of the arena fixture). One assertion in `golden_frame.rs` was re-anchored — its
"empty floor" sample at (475, 20) now sits under the enlarged top-edge ruler glyph
(deliberate, the 9.5 px floor), so the sample moved to (475, 40); the Asteroid pixels
themselves are unchanged, verified by a pixel diff of before/after frames.

### Simulation behaviour

Byte-identical. `neuroarena-headless` output compared against `cf8e097` (throwaway
worktree):

```
seed 2026, 12 Generations, 1 worker   → identical
seed 7,     8 Generations, 4 workers  → identical
```

The `sim` crate has no changes at all in this pass.

### Performance — Apple M5, release, same seed and step

`render_ms` = p50/p95 of 120 renders after 10 warm-ups (visual_probe). p95 swings 2–4.7 ms
run to run on this machine, so ranges from repeated runs are the honest numbers.

| State                                | Before p50 / p95      | After p50 / p95       |
| ------------------------------------ | --------------------- | --------------------- |
| 1440×900 (5 baseline / 4 after runs) | 1.00–1.37 / 1.99–4.28 | 0.99–1.31 / 2.03–4.01 |
| 1000×620                             | 0.878 / 1.903         | 0.932 / 2.263         |
| 900×560 (minimum)                    | 0.862 / 1.845         | 0.876 / 1.900         |
| 2880×1800 @2×                        | 2.028 / 3.319         | 2.174 / 3.489         |
| dense fixture                        | 1.010 / 2.219         | 1.054 / 2.257         |
| impact frame                         | 1.141 / 3.394         | 1.073 / 2.639         |
| 151-generation chart                 | 1.108 / 3.381         | 1.187 / 3.279         |

No material regression; the impact frame actually got cheaper (the IGN composite replaced
a hash chain). The paused idle path takes the loop from continuous to **0.0 % CPU**
(instantaneous `top` samples after `space`, resume measured at ~1.0 %).

### Visual inspection

Inspected through matched before/after offscreen captures driving the real Renderer at
1440×900, 1000×620, 900×560, 2880×1800@2×, 2560×1080, plus the dense, impact and
150-generation fixtures and a 5× crop of the Chart. Independent vision reads of the pairs
confirm: the minimum window's rulers and NEAREST HULL / V / T readouts are legible (4 px
before); all three strip sections present at 900×560; at 2560×1080 the strip's edges
coincide with the field's and the motor tiles are ~120 px; the Chart shows
`0 / 0.1 / 0.2 / 0.3` labelled gridlines, the band, both legend marks, and the newest
value `0.30` in the heading row; the focused button carries a ring outside its face
(before/after crops in the evidence directory).

The live application was launched in release mode and ran 1 m 32 s at Generation 104 with
no GPU error; a full-screen capture recorded the populated Chart (band + mean + legend)
and the anchored strip in the EVOLVING state (`live-full.png`). A second session was
paused with `space`, the lamp read back as **PAUSED** from the image, and the capture
recorded the paused frame while the loop held still at 0.0 % CPU (`live-paused.png`).
`screencapture` worked this session, so **native window presentation is verified** this
pass, closing the gap the luminous-field report recorded.

## 6 · Limitations and what was not verified

- **The Chart's band is honest but blocky at one Wave.** Early Generations have
  median = p90 = 0 or a 0→1 step, so the band is a columnar wash; the mean stays the
  smooth read. Judged acceptable (the counts are integers) rather than smoothed.
- **The interaction-state captures replicate the ring's geometry, not its code path** —
  the offscreen probe drives the panel functions directly and `draw_focus_ring` lives
  inside `build_frame`; its geometry is pinned by unit tests, and the live window was
  verified running, but a clicked-through capture of the real ring is still indirect.
- **Golden frame on a second GPU** remains unverified (unchanged from prior passes).
- **Network bundling (research R15) and the sensorium time-strip (R10) were researched,
  not built** — the pass held to the audit's five highest-impact changes plus the light
  polish; both remain the obvious next candidates.
- **Hover/focus under a real pointer** was exercised live via the new focus ring and the
  live session, but a pointer-driven capture of hover-on-button was not produced.
