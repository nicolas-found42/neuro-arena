# Material and light — a research ledger

2026-09-13 · native Rust · wgpu 30.0.1 / winit 0.30.13 / lyon_tessellation 1.0.22 · Metal (macOS)

## Question and scope

What are the best open-source techniques to make 2D polygonal rock/ship/space rendering
in NeuroArena read as real material and real depth, that fit a
CPU-tessellated-triangles-plus-one-fullscreen-shader-pass architecture, without an engine
migration and without fighting the constraints the project has already committed to in
its own ADRs? Five sub-questions, each researched as its own evidence chain
(awesome-list → project → the actual implementation file, pinned to a commit or tag,
license verified on that specific file):

1. Making convex/irregular 2D polygons read as lit 3D mass.
2. Procedural rock/crater/surface detail in a fragment shader.
3. Volumetric/parallax depth for the starfield-and-nebula backdrop.
4. Distance-field 2D rendering for crisp shapes at any zoom.
5. Anti-aliasing / line quality for thin strokes.

Every technique below is reported against one schema: user-visible improvement;
discovery source; repo + a direct link to the implementation file pinned to a commit or
tag; what the code actually does, in enough detail to reimplement in WGSL; the license of
that specific file, verified; adopt-dependency vs. adapt-source vs.
independently-implement; integration risk; and an _estimated_ (never measured — this repo
reserves "measured" for numbers actually taken on real hardware) per-frame GPU cost at
1440×900 and 2880×1800.

Method followed for every topic: start at
[mikbry/awesome-webgpu](https://github.com/mikbry/awesome-webgpu) and
[terkelg/awesome-creative-coding](https://github.com/terkelg/awesome-creative-coding),
then awesome-wgpu, awesome-graphics-programming, awesome-shaders and awesome-gamedev
(plus topic-specific lists such as awesome-godot or awesome-rust-gamedev where they
surfaced a real lead). An awesome-list entry or a README is a lead, not a citation — every
row below was followed into the actual shader/source file before it was reported.

## 0 · Existing architecture (verified from source, 2026-09-13)

This section is load-bearing for every "integration risk" judgment below, so it is
recorded once here rather than re-derived per topic.

- **Pipeline.** Every shape is CPU-tessellated into triangles by `lyon_tessellation`
  (fill tessellator for filled polygons, stroke tessellator for lines), rasterized with
  hardware 4× MSAA into a `Rgba16Float` HDR scene target (`HDR_FORMAT` in
  `app/src/renderer.rs`), then resolved through a half-resolution bloom mip chain and
  composited into the window's sRGB surface. Glyphs are drawn last, straight into the
  composite, and never bloom or blur. All of this is `app/src/shader.wgsl` (356 lines) —
  one module, five kinds of draw (`vs_shape`/`fs_shape`, `vs_fullscreen`/`fs_backdrop`,
  `fs_bloom_*`, `fs_composite`, `vs_text`/`fs_text`).
- **`fs_shape` is a pure passthrough.** It returns the interpolated per-vertex colour
  unmodified (`app/src/shader.wgsl:64-67`). There is no per-fragment lighting, noise, or
  distance-field math anywhere in the fragment shader today for shapes.
- **Rock lighting already exists — on the CPU, per vertex.** `app/src/scene.rs`
  (`draw_asteroids`, ~lines 472-660) computes a fake normal per vertex as the unit
  direction from the asteroid's own centre to that vertex, dots it against a fixed key
  light `KEY_LIGHT = normalize(-0.6, -0.6)`, shapes the terminator with `shade^1.8`
  (`ASTEROID_TERMINATOR = 1.8`), floors the dark side at `ASTEROID_MIN_SHADE = 0.04` so
  ambient (`ASTEROID_AMBIENT = 0.16`) keeps it legible as matter rather than a hole, gives
  every facet of one rock a single shared core colour (`ASTEROID_CORE_SHADE = 0.55`) so
  the triangle fan's centre doesn't pinwheel, and inks the facet _edges_ three ways by the
  same dot product: a hot rim where a facet's outward direction is within a cone of the
  light (`cos > RIM_CONE = 0.866`), a cool desaturated back edge where it faces away
  (`cos < BACK_CONE = -0.5`), and a plain silhouette edge otherwise. Three straight-line
  "cracks" per rock run from near-centre to the rock's own vertices, alpha-modulated by
  the same `shade()` function. **This already is the "faceted flat-shaded fake-normal
  2.5D" technique** the brief asked about — every recommendation below is judged against
  it as prior art, not as a gap to fill from zero.
- **The nebula/starfield backdrop is one fullscreen pass reading a CPU-baked texture,
  deliberately.** `fs_backdrop` (`app/src/shader.wgsl:104-137`) reads a single 256×256
  RGBA8 density texture baked once at init by `app/src/deepfield.rs`, and does a vertical
  wash/vignette lerp, two squared density samples added at fixed gains, and a rectangular
  vignette — a handful of samples and lerps, no raymarching, no per-fragment noise, no
  animation. The reason it is baked rather than evaluated live is stated directly in the
  source: _"a golden frame has to come out the same on the machine that wrote it and on
  the machine that checks it. `sin`-based hashes and `fract` of large arguments are the
  two places shader noise drifts between GPUs; integer hashing on the CPU cannot."_
  (`app/src/deepfield.rs:7-11`). The project backs this with byte-identical golden-frame
  regression tests (`golden_frame.rs`) and a deterministic offscreen probe used for every
  capture in its own research docs. **Any live per-fragment procedural noise proposed
  below (topics 2 and 3 especially) has to answer to this constraint explicitly** — it is
  the single biggest integration-risk factor in this document, ahead of raw performance.
- **The time uniform is wired but dead.** `Frame`'s WGSL layout documents `params.x` as
  "time (seconds)" (`app/src/shader.wgsl:19`), but the Rust side hard-codes it to `0.0`
  today: `params: [0.0, BLOOM_INTENSITY, GRAIN, 0.0]` (`app/src/renderer.rs:488`). Any
  time-driven effect (twinkle, flow, animated grain) needs this wired to a real clock as
  a small but real prerequisite, and that clock must freeze cleanly when the app is
  paused — the project's presentation loop already treats "a paused, settled frame
  produces cheap, byte-identical output" as an invariant worth its own fix (see
  `docs/research/observatory-next.md`, "No idle path").
- **Lyon's stroke tessellation is already adaptive.** `app/src/vector.rs:26` sets
  `StrokeOptions` tolerance to `0.25 / current_display_scale` — it already tightens at
  higher DPI/zoom rather than using a fixed tolerance. Thin strokes (facet rims, cracks,
  instrument lines, sensor rays) are a first-class, frequently used primitive, not an
  edge case.
- **No texture assets, by policy as much as by fact.** The only texture in the whole
  project is the one CPU-baked density texture above; everything else is vector/procedural.
  A prior research pass explicitly rejected hand-authored sprite/texture assets as a
  general upgrade path. A CPU-baked _procedural_ lookup texture generated by this
  project's own code (extending the `deepfield.rs` pattern) fits everything already
  built; an imported PNG (a matcap, a height-mapped crater field, a starfield photo) does
  not, regardless of its license.
- **Bloom stays exactly at white.** The threshold is `BLOOM_THRESHOLD = 1.0` with a soft
  knee of `0.55` (`app/src/shader.wgsl:151-161`); PBR Neutral tone-mapping
  (`resolve_bloom`) is applied to the bloom term only, never to the base scene or to text.
  **No technique in this document may require lowering that threshold or blooming the
  whole frame** — every new glow has to be authored the same way existing ones are,
  by writing a shape's colour past 1.0 in the HDR target via the existing per-vertex
  `gain` mechanism (`Painter::gain`, ADR 0011).
- **Standing rejections this document does not re-litigate without saying so:**
  **`vello`** as a dependency or engine — `docs/adr/0011-high-dynamic-range-frame-with-bloom.md`
  and `docs/adr/0004-native-macos-application-in-rust.md` reject it on a wgpu-major-version
  pinning conflict ("an engine is a migration, not a feature"); its _technique_ is fair
  game to cite, never the crate. **thebookofshaders.com** — all-rights-reserved, verified
  by a prior pass; reference-only at most, never copied from. **Shadertoy's default
  license** — "(c) All rights reserved" unless the specific shader page shows an explicit
  permissive badge; every Shadertoy link below is reference-only unless a permissive
  badge was personally confirmed on that exact page. **Temporal auto-exposure** — breaks
  paused-frame identity. **Motion blur / depth of field, sprite assets** — rejected on
  the same "stay procedural, stay honest-instrument" grounds as texture assets above.
- **Cost-measurement convention carried over from this repo's own docs:** a number is
  called "measured" only when it was actually run on real hardware (this repo's own
  anchor: the whole HDR-plus-bloom-plus-composite chain measured **+0.31 ms at 1440×900**
  and **+0.68 ms at 2880×1800** on an Apple M5, release build, same seed and step, against
  a 16.7 ms/frame budget at 60 Hz — `docs/adr/0011-high-dynamic-range-frame-with-bloom.md`).
  Every cost figure below is an _estimate_ reasoned from tap/instruction/step counts and
  resolution, labelled as such, and never claims to be a measurement.

---

## 1 · Making convex/irregular 2D polygons read as lit 3D mass

_(pending — background research in progress)_

## 2 · Procedural rock/crater/surface detail in a fragment shader

_(pending — background research in progress)_

## 3 · Volumetric / parallax depth for the starfield-and-nebula backdrop

_(pending — background research in progress)_

## 4 · Distance-field 2D rendering for crisp shapes at any zoom

_(pending — background research in progress)_

## 5 · Anti-aliasing / line quality for thin strokes

_(pending — background research in progress)_

---

## Ledger summary

_(pending — filled in once all five topics have returned)_

## What could not be verified

_(pending)_

## Recommendations, ranked by visual impact × feasibility

_(pending)_
