# NeuroArena — deep-field observatory

2026-09-13 · Apple M5 · macOS 26.4 · Rust 1.98.1 · native winit / wgpu / Metal

## Result and scope

The running application's presentation was upgraded from the flight-observatory
instrument into a **deep-field observatory**: the Arena is now a lit place with
an event language of light, and the sidebar carries crafted chrome — with the
simulation untouched and byte-identical outputs. Launch with
`cargo run --release --bin neuroarena`. Nothing was pushed or committed.

![Before: the flight observatory baseline](../visuals/2026-09-deep-field/before.png)

![After: the deep-field observatory](../visuals/2026-09-deep-field/after.png)

Both captures: seed 2026, Generation 1, member 0, fixed step 180, 1440×900 at
1× display scale, paused presentation, Rays on, produced by the deterministic
offscreen probe (`visual_probe`) driving the real renderer.

## Direction

Three directions were weighed against the audit:

1. **Luminous arcade** (bloom post-processing, scanlines, saturated flashes) —
   rejected: the prior pass correctly identified the telemetry density as the
   app's value; heavy post-processing fights it and costs bandwidth.
2. **Organic computation** (force-directed networks, flowing generative forms) —
   rejected: moving node positions impede comparison and read as invented
   neural dynamics.
3. **Deep-field observatory — selected.** The instrument DNA stays (cyan
   perception, amber energy, white numbers, honest telemetry); the Arena gains
   atmosphere and material, events gain bounded additive light, and the
   interface gains quiet craft.

The signature element is **light as the event language**: the watched Ship's
trail is a fading light trace, bullets are tracers, impacts are gold flashes,
a new Wave pulses the containment seam, and death scatters embers. All of it
is additive light drawn from real simulation events — nothing decorative is
labelled as measurement.

## Audit findings this pass addressed

| Priority | Finding (evidence)                                                                                                        | Resolution                                                                                                                                                                                      |
| -------- | ------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| High     | The Arena read as an empty void: flat `ARENA_BG`, one soft glow, a single 90-star layer, faint grid (`scene.rs` baseline) | Layered backdrop: vertical wash, three LCG-seeded nebula glows, three-depth starfield with cross-sparkle near stars, corner vignette                                                            |
| High     | Asteroids were flat gray polygons; danger and material did not read (`draw_asteroids` flat fill)                          | One key light (upper-left): per-facet gradient shading `ASTEROID_UNLIT`→`ASTEROID_LIT`, rim-light strokes on lit facets, shading-modulated crack strokes; sim polygons untouched                |
| High     | Events (impact, wave change, death) had minimal visual language (arc+tick echoes only)                                    | Bounded additive events: gold flash + ring + deterministic sparks (impacts), seam light pulse (wave increment), ember burst (death), all capped and lifetime-bounded                            |
| Medium   | Ship plume and bullets were barely visible; trails were faint polylines                                                   | Plume and tracers moved to the additive pass with flame-core gradient; trail restyled as a tapering luminous trace (same sampling/lifetime/seam rules)                                          |
| Medium   | Sidebar panels were uniform boxes; no section rhythm                                                                      | Accent tick + fading hairline per panel, corner brackets, HUD accent bar and dividers, chart area fill + glowing latest point, filled slider, styled generation banner with fade, wordmark mark |
| Low      | Cursor gave no affordance over controls                                                                                   | Pointer cursor over hot controls (`appstate::on_move`)                                                                                                                                          |

## Foundation: the additive luminous pass

The enabling change is a second geometry buffer in the `Painter`
(`luminous`), drawn by a new additive pipeline (`SrcAlpha, One`) between the
alpha-blended triangles and the glyphs inside the existing single 4× MSAA
render pass — no post-processing, no extra render target. Feasibility was
proven on the real GPU before any slice built on it (probe capture, pixel
diffs showed purely positive additive deltas, no frame-time cost).

**A real defect was caught and fixed during verification:** the additive
blend initially used `alpha: REPLACE`, so every light dragged the framebuffer's
alpha channel toward its own low alpha — invisible in the opaque window, but
every capture path (probe PNGs, goldens, screenshots) composited lit regions
as transparent holes. The fix accumulates alpha (`One, One, Add`), after which
the whole frame is opaque (verified: minimum alpha 255 across the frame).

## Research chain and adoption decisions

Discovery followed the mandated lists —
[awesome-rust](https://github.com/rust-unofficial/awesome-rust),
[awesome-webgpu](https://github.com/mikbry/awesome-webgpu),
[awesome-creative-coding](https://github.com/terkelg/awesome-creative-coding)
— plus [awesome-wgpu](https://github.com/rofrol/awesome-wgpu),
[awesome-gamedev](https://github.com/Calinou/awesome-gamedev) and the repo's
own [graphics synthesis](SYNTHESIS-GRAPHICS.md). Evidence chains were followed
into implementation files, pinned to the versions in use:

- **vello — rejected.** vello 0.10.0 pins wgpu 29.0.3 while this app is on
  wgpu 30.0.1 (wgpu-30 support is still an open PR line). It cannot share the
  existing device/encoder, and the custom pipeline already provides
  per-vertex gradients. Extending the in-tree pipeline won.
- **wgpu 30 built-ins — verified and adopted.** Additive blending inside a
  4× MSAA pass with a resolve target is supported (blend happens before
  resolve); the probe proved it on the Apple M5. No bloom/blur chain was
  needed, so no offscreen sampling refactor was required.
- **Tier-0 patterns from the repo's own synthesis — adopted as the arena
  spec:** flame gradient (pale core `#fff8a3` → amber), layered starfield,
  additive debris/flash loops, crater/crack detailing, vignette. These were
  validated in the browser prototype era; they transfer directly because they
  are technique patterns, not code.
- **bevy_hanabi-style stretched tracers — adapted** (previous-segment
  stretching along velocity) for bullet streaks, implemented as gradient quads.
- **Icon fonts (Tabler/Phosphor/codicons/Material Symbols) — rejected:**
  licensing and coverage traps (no radar glyph; generated TTFs not committed;
  CC-BY attribution; 10 MB variable fonts cosmic-text cannot steer), and
  geometric Painter marks are more coherent with the instrument language.
- **`palette` crate — rejected; Oklab hand-roll — deferred.** The ramps in
  this pass are low-alpha light fades where sRGB-space `Rgba::mix` proved
  sufficient; Oklab remains the right tool if semantic color ramps (e.g.
  fitness heat) land later (~40 lines, no dependency).
- **winit 0.30 capabilities — adopted selectively:** pointer cursor icons.
  IME/custom-cursor capabilities verified but not needed.

**Dependencies added: none.** The upgrade is built entirely on the existing
wgpu/lyon/cosmic-text stack; the license surface and `cargo deny` status are
unchanged. Fonts are unchanged (JetBrains Mono, Space Grotesk, OFL-1.1).

## State, correctness and resource rules

- `sim/` is untouched (`git diff sim/` is empty): physics, fitness, streams,
  save format and determinism contracts are intact by construction. A headless
  oracle run (seed 2026, 3 Generations, 1 worker) reproduces the exact
  **432,325 steps** and generation results recorded by the previous audit.
- All animation is driven by `world.time` (sim time): pausing freezes plume
  flicker and events; nothing uses the wall clock. Cosmetic randomness is
  LCG/integer-hash local to the app (separate seeds from the starfield
  pattern); the simulation's `Rng` is never drawn.
- Reduced motion (`m`): plume flicker, wave pulses and death bursts are
  suppressed; trails/echoes follow the existing rules; the banner skips its
  fade; all telemetry stays on. Verified live in the running app.
- Every new container is bounded: impacts 24 @ 0.55 s (unchanged), wave
  pulses 3 @ 0.9 s, death bursts 2 @ 0.7 s; cosmetic events stay suppressed
  above 16× speed. Seam-copy + clip discipline is kept for every arena-space
  effect; trails and sparks never draw across-seam streaks.
- Triangle count **dropped** ~40% (3,232 → 1,955 in the normal scene):
  gradient fans replaced stroke-quad fans in the asteroid and ship work, more
  than paying for the backdrop layers.

## Verification

```sh
cargo fmt --all -- --check                 # clean
cargo clippy --workspace --all-targets -- -D warnings   # clean
cargo test --workspace                     # 12 suites, all pass (goldens included)
cargo build --workspace --release          # clean
cargo +1.89.0 check --workspace --all-targets --locked  # clean (MSRV)
```

The golden fixture was regenerated deliberately after visual inspection
(`NEUROARENA_WRITE_GOLDEN=1`), tolerance unchanged. New unit tests cover:
light/matter buffer separation and clip parity, gradient fan corner colours,
impact/wave/death event detection and bounds (no spurious fire on key change),
banner fade scaling, chart fill column bounds, panel chrome containment.

### Performance (renderer submission, 120 warmed samples; probe methodology unchanged)

| Capture                | Before p50 / p95 (ms) | After p50 / p95 (ms) |
| ---------------------- | --------------------- | -------------------- |
| Normal, 1440×900       | 0.785 / 1.815         | 1.012 / 2.674        |
| Impact frame           | 0.651 / 1.357         | 1.242 / 3.136        |
| Dense topology fixture | 0.650 / 1.410         | 0.711 / 1.680        |
| Small, 900×560         | 0.473 / 1.425         | 0.514 / 1.867        |
| Retina, 2880×1800 @2×  | — (no fresh baseline) | 1.202 / 2.274        |

All runs sit far inside the 16.7 ms/frame budget. These are single runs with
visible scheduler variance (the baseline itself measured 0.650–0.785 p50
across invocations); they support a budget check, not a precise delta claim.
The impact frame carries the boosted flash and is the most expensive state
measured.

### Native inspection

The release binary was launched and inspected in motion: ordinary evolution at
1× and >9,000× measured speed, the generation banner between generations, the
chart filling with history, bullet tracers and impact brackets in flight,
reduced-motion toggling live, and the small-window layout. Reproducible
captures below are offscreen renders of the same native stack.

Files under `docs/visuals/2026-09-deep-field/`:

- `before.png` / `after.png` — matched normal scene
- `impact.png` — a real seeded collision with flash, ring and sparks
- `dense.png` — 48-hidden-node topology stress fixture
- `small.png` — 900×560 (named output snapshot layout)
- `retina.png` — 2880×1800 @2×

```sh
cargo run --release -p neuroarena-app --example visual_probe -- /tmp/f.png 1440 900 180
cargo run --release -p neuroarena-app --example visual_probe -- /tmp/f.png 1440 900 impact
cargo run --release -p neuroarena-app --example visual_probe -- /tmp/f.png 1440 900 180 1 0 0 dense
cargo run --release -p neuroarena-app --example visual_probe -- /tmp/f.png 2880 1800 180 2
```

## Remaining limits

- The Wave seam pulse and death burst are unit-tested and code-reviewed; they
  are too transient to capture as stills (states the probe does not stage)
  and were not identified in the live captures reviewed.
- No video recording API exists in the app; evidence is stills plus live
  inspection.
- The image-preview tool used during development renders near-black additive
  gradients with severe artifacts; framebuffer correctness was established by
  direct PNG pixel inspection instead.
- Renderer timing is submission-only (no end-to-end display latency claim),
  matching the prior audit's methodology.
