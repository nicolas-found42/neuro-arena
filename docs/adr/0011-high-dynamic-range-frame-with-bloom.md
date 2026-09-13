# ADR 0011 — A High-Dynamic-Range Frame, and Bloom Separated by Value

- **Status:** Accepted — supersedes the "no post-processing" presentation choice recorded in the deep-field observatory pass (`docs/research/deep-field-observatory.md`).
- **Date:** 2026-09-13

## Context

The deep-field pass gave the Arena an event language of light: plumes, tracers, impact flashes, seam pulses and the watched Ship's trace all go into a second geometry buffer that is blended additively over the frame's matter. That pass explicitly considered and rejected bloom post-processing as "heavy post-processing [that] fights [the telemetry density] and costs bandwidth", and kept a single render pass straight into the window's sRGB surface.

The rejection was right about the risk and wrong about the cause. What makes bloom fight a dense instrument panel is applying it to the _whole frame_: a white number at 0.90 sRGB and a bullet's core are the same value in an 8-bit sRGB target, so any threshold that halos the bullet also smears the number. The usual answer is a mask or a second scene pass, and both are the kind of machinery the rejection was protecting the app from.

Meanwhile the cost of _not_ having it was visible. In the pre-change capture (`docs/visuals/2026-09-luminous-field/before.png`) a bullet, a thrust plume and an Asteroid coming apart are all shapes painted in bright colours. Nothing in the Arena reads as a source of light, because in an 8-bit target nothing can: white is the ceiling, and the ceiling is also where the interface's text lives.

## Decision

The frame is assembled in high dynamic range and resolved into the window at the end:

1. **Scene** — 4× MSAA `Rgba16Float`. A procedural backdrop paints the deep field, matter is alpha-blended over it, light is added over that. Because the target is floating point, an emitter can be written _past_ white.
2. **Bloom** — a half-resolution mip chain over the resolved scene: a thresholded 13-tap downsample with a Karis average on the first level, then a 3×3 tent upsample adding each level back into the one above it.
3. **Composite** — into the window's sRGB surface: the scene plus the bloom, then the glyphs, which are drawn last and therefore never bloom or blur.

The threshold sits at exactly white, and every colour the interface is authored in is at or below white. **The separation is carried by the values themselves**, so there is no mask, no second scene pass and no per-draw flag. What decides whether something glows is one number per emitter — its gain in `theme::light` — saying how far past white it is written.

## Considered Options

- **Keeping the single-pass frame** — rejected: it is the reason light and paint are the same thing on screen, and no amount of colour work inside an 8-bit ceiling fixes that.
- **Bloom over the whole frame with a threshold below white** — rejected: this is the failure the deep-field pass correctly predicted. It smears the panel text, and it has to be bought back with a mask.
- **A separate emissive render target** — rejected: a third full-size target and a second pass over every light, to encode information that one float per vertex already carries.
- **Adopting a renderer that brings this with it (`vello`, a full engine)** — rejected again, for the reason the deep-field pass gave: `vello` pins a wgpu major version this workspace is not on, and an engine is a migration, not a feature. The chain above is about 200 lines of WGSL and 250 of Rust.

## Consequences

`Renderer::render` no longer takes a clear colour; it takes a `Frame` instead — the viewport _and_ the Arena's rectangle inside it — because the backdrop pass paints both grounds and needs to know where the boundary is. The golden frame and the offscreen probe were updated for that signature, and the golden image was regenerated: the Arena looks different on purpose, and the image records what it looks like now.

`Painter` gains one piece of frame state, `gain`, alongside the transform and the clip. It multiplies the linear colour of light only; matter ignores it, and `clear` resets it.

The interface palette acquires a hard constraint: **nothing the interface prints may exceed white**, or it will start to halo. `theme::tests::nothing_the_interface_prints_can_bloom` pins it, and `app/tests/hdr_bloom.rs` pins the end-to-end property on the real GPU — a light halos, printed white does not.

Measured cost on an Apple M5, release, at the same seed and step: +0.31 ms per frame at 1440×900 and +0.68 ms at 2880×1800, against a 16.7 ms budget at 60 Hz. The simulation is untouched: the same seeds produce byte-identical headless output.
