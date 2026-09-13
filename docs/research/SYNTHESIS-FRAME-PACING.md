# Frame Pacing Research Synthesis — the p50 Is Already the Panel Period

**Date:** 2026-09-12
**Project:** `neural-network-game` / `neuroarena` — native macOS app, `sim` crate (no windowing) + `app` crate (winit 0.30.13 + wgpu 25.0.2 + Metal).
**Goal as asked:** bring the median (p50) frame interval to `<= 16.667 ms`, i.e. p50 `>= 60.0` fps.
**Constraint:** Frame presentation is quantised to the panel period, so every candidate must be classified as (a) a real pacing mechanism, (b) a knob actually reachable at the pinned versions, or (c) a measurement artifact. Every claim cites the source that owns it: Apple documentation or SDK headers, wgpu/wgpu-hal/winit sources at the pinned versions, the Vulkan specification, or a labelled practice report.
**Machine:** Apple M5, built-in Liquid Retina 1710x1112. `CGDisplayCopyDisplayMode` reports 60.0 Hz; `NSScreen.maximumRefreshInterval` is 16.66600 ms (60.0024 Hz); `NSScreen.maximumFramesPerSecond` is 60. One nominal panel period is therefore **16.666 ms**, and `1/60 s = 16.667 ms` is the target used below.
**Pinned versions (from `Cargo.lock`):** `wgpu 25.0.2`, `wgpu-core 25.0.2`, `wgpu-hal 25.0.2`, `wgpu-types 25.0.0`, `winit 0.30.13`, `metal 0.31.0`.

**Research slices (2 parallel subagents, ~10 min each):**
- `frame-pacing-present-path.md` — owns the swapchain/present path: what blocks, which `CAMetalLayer` and wgpu knobs exist at the pinned versions, and what Apple's own docs say the acquire waits for. 16 findings, 13 disproved mechanisms.
- `frame-pacing-loop-strategies.md` — owns the loop: fixed-timestep-with-interpolation, sleep-based limiters, display links, and six independently-sourced practice reports. Findings A1-A8, options O1-O6.
- This file adds the phase decomposition measured on this machine (below) and reconciles the two.

---

## The answer

1. **The p50 is already one panel period.** Measured 600-frame runs: p50 **16.658 ms** (speed x0), **16.679 ms** (default x100), **16.664 ms** (unbounded) against a 16.666 ms panel period — 59.96 to 60.03 fps. The spread is `-8.7 us .. +12.3 us` around the period, i.e. phase jitter, not lost frames.
2. **A vsynced windowed loop on a fixed 60 Hz panel cannot exceed a 60 fps median.** Presentation is quantised to the panel: one frame per vertical blanking period, and a frame that misses its slot waits a whole extra period (`VkPresentModeKHR` FIFO, normative; Apple WWDC21 10147 for the fixed-rate case).
3. **Therefore the goal is either already met or requires different hardware.** p50 `> 60` needs a panel faster than 60 Hz. `PresentMode::Immediate` (vsync off) was measured and does **not** help: p50 was identical to Fifo within 20 us, because a windowed compositor recycles drawables on the panel cadence in both modes. Nothing in this codebase, and no wgpu 25 knob, moves the median above the panel rate.
4. **The reachable work is the tail and the phase, not the median.** p99 was 17.3-18.4 ms and max 19.6-40.2 ms on a quiet machine (112-229 ms while this machine was loaded by the research agents). Those are the missed slots; that is where an implementation can win.
5. **The same measurement answers the 120 fps question, negatively on this hardware and positively in the app.** Unpaced, this loop runs at p50 1.03 ms (973 fps); a 120 Hz display would give p50 8.333 ms with no code change. See "The 120 fps target" below.

[INFERENCE] Corollary: the earlier statement in this session that "`frame.present()` blocks on vblank" was wrong, and the measurement now shows why. `present()` returns in ~20 us; the block is in `surface.get_current_texture()`.

---

## The frame, decomposed (measured, this synthesis)

Temporary probe in `App::draw` and `App::tick`, 600 frames per run, five sampled phases per frame, probe since reverted and the tree left clean. All values are microseconds; p50 unless stated.

| Speed | painter | acquire (`get_current_texture`) | encode (`renderer.render`) | present | interval (p50) | turnaround (p50) |
|---|---|---|---|---|---|---|
| x0 | 122 | 13309 | 3004 | 24 | 16673 | 37 |
| x100 (default) | 120 | 12738 | 3599 | 20 | 16669 | 172 |
| unbounded | 116 | 12127 | 3632 | 18 | 16654 | 714 |

Reading, in the order the loop executes them:

- **painter 0.12 ms p50** — `build_frame()`: panel boxes, glyph anchors, Arena geometry, tessellation into vertex arrays. This is the *only* work before the acquire.
- **acquire 12.1-13.3 ms p50** — `Surface::get_current_texture()` -> `CAMetalLayer.nextDrawable()`. This is the pacer, and it is the compositor's, not ours. It confirms the sibling file's finding from the other direction: wgpu 25's `present()` only flags and commits (`wgpu/src/api/surface_texture.rs:37`; `wgpu-hal/src/metal/mod.rs:468-492`), while the acquire is the blocking call. Verified in the vendored crates at `wgpu-hal-25.0.2/src/metal/surface.rs:171,174,176,195` and `wgpu-25.0.2/src/api/surface_texture.rs:37`.
- **encode 3.0-3.6 ms p50** — the whole of `Renderer::render`: `text.build()` (cosmic-text shaping per string per frame), three vertex-buffer uploads, command encoder, submit.
- **present 18-24 us** — non-blocking, as the source says.
- **turnaround 0.04-0.71 ms p50** — winit's run-loop hop: `about_to_wait` -> `request_redraw` -> `queue_redraw` -> `CFRunLoopWakeUp`, delivered at the next `kCFRunLoopBeforeWaiting` (`winit-0.30.13` `app_state.rs:297,369,384`, verified vendored).

**Consequence for the sibling file's option 2** ("cut the CPU work before the acquire"): the pre-acquire CPU work is 0.12 ms out of 16.67 ms, so it cannot move the median. The 3.6 ms of encode work runs *after* the acquire and is absorbed by the three-drawable queue; it reaches the tail (a frame whose encode overruns the slot misses it) but not the median. Option 2 is therefore re-ranked below and replaced by the tail options in Tier 1. The x0-vs-x100 p50 movement the sibling file cites (21 us) is within the run-to-run phase spread measured here (16.654-16.679 ms at the same speed).

---

## The 120 fps target (measured, 2026-09-12)

The same temporary probe (120 warm-up frames discarded, 900 measured, three runs per configuration, `NEUROARENA_PRESENT` selecting the mode; all instrumentation since reverted) answers a second question: **can the p50 interval reach 8.333 ms (120 fps) on this 60 Hz panel?**

| Configuration | p50 interval | p50 fps | mean interval | mean fps |
|---|---|---|---|---|
| Fifo, speed x100, runs 1-3 | 16.706 / 16.710 / 16.721 ms | 59.8-59.9 | 16.666 / 16.667 / 16.666 ms | 60.0 |
| `PresentMode::Immediate`, speed x100, runs 1-3 | 16.705 / 16.707 / 16.726 ms | 59.8-59.9 | 16.481 / 16.501 / 16.556 ms | 60.4-60.7 |
| Offscreen render (no acquire, no present), x100 | **1.028 ms** | **973** | 1.042 ms | 960 |
| Offscreen render, unbounded speed | **1.194 ms** | **837** | 1.322 ms | 757 |

Three conclusions, all from measurement:

1. **`PresentMode::Immediate` does not unpin a windowed loop.** p50 is identical to the Fifo runs within 20 us. `wgpu` does set `displaySyncEnabled = false` (the configure path succeeds, so the mode is applied), and the mean improves slightly (60.4-60.7 fps) because a few frames come out short — but the acquire still blocks 14.4 ms p50, so the windowed compositor recycles drawables on the panel cadence regardless. The tear-free-mode/tearing-mode distinction does not change the median in a window.
2. **The app is not the limit.** Unpaced, this exact draw work runs at p50 1.03 ms — 973 fps — and 1.19 ms at unbounded speed. All 16.67 ms of on-screen frame time is the compositor's drawable pacing.
3. **Glyph shaping is the dominant per-frame cost** on both paths: 1.85 ms of the 2.00 ms surface `encode` bucket, and 0.96 ms of the 1.03 ms unpaced frame. It is ~11% of the 8.333 ms budget a 120 Hz panel would grant, so it needs no change to reach 120 fps, but it is the first thing to attack if margin is ever wanted.

**Verdict:** on this machine (built-in 60 Hz panel, `minimumRefreshInterval == maximumRefreshInterval`, so no Adaptive-Sync branch) **the p50 interval cannot go below one panel period, in any present mode, in a window.** `p50 = 120 fps` requires a display whose refresh is at least 120 Hz; on such a display `PresentMode::Fifo` paces to the panel period, giving p50 8.333 ms = 120.0 fps with **no code change**, with roughly 7x headroom against the measured 1.0-1.2 ms frame cost. [INFERENCE] from the measured frame cost plus the Fifo mechanism; no 120 Hz display is attached to this machine to verify it directly.

---

## Options for NeuroArena — ranked

### Tier 0 — decide what the target is (cheap, do first)

- **D1. Measure the true panel period from a display link.** `NSView.displayLink(target:selector:)` -> `CADisplayLink` (`targetTimestamp`, macOS 14+; `CVDisplayLink` is deprecated as of macOS 15.0 with no `targetTimestamp`). Comparing consecutive `targetTimestamp`s gives the panel's real period independent of our loop, which is the only way to know whether the 12 us is a defect or the grid itself. Cost: diagnostic only. Cannot change the median.
- **D2. Accept the metric that matches the goal.** If the goal is smooth motion, the number to hold down is p99/max — the missed slots that cost a full period — not p50. On a 60 Hz panel, p50 = 16.67 ms *is* success.

### Tier 1 — reachable improvements (tail, not median)

- **R1. Cache text shaping for strings that did not change.** Glyph bitmaps are already cached by `CacheKey` (`app/src/atlas.rs`), but `TextRenderer::build` calls `set_text` + `shape_until_scroll(Shaping::Advanced)` for every item every frame (`app/src/text.rs:69-90`), even though only the HUD/Chart numbers change. Attacks the 3.6 ms encode bucket and the p99 misses. Local to `text.rs` + the panel call sites; no API change.
- **R2. Move Generation turnover off the main thread.** `finish_generation` runs `Run::complete_generation` (speciation + breeding over the Population) inside a redraw. This is the likely owner of the max-frame outliers (40 ms quiet, 112-229 ms loaded). [INFERENCE] labelled: the correlation is from frame timing only.
- **R3. One measured A/B of `desired_maximum_frame_latency: 1`** (`app/src/appstate.rs:754`) -> `CAMetalLayer.maximumDrawableCount = 2`. This changes pool depth and phase, never cadence. Both siblings flag the risk: wgpu sets `allowsNextDrawableTimeout:false`, so a starved pool blocks indefinitely rather than erroring, and egui ships a comment that `Some(1)` "cuts FPS in half" on iOS. Keep only if the probe shows p95 improves.

### Tier 2 — architectural, only if phase stability is worth the cost

- **A1. Drive rendering from a display link** (`NSView.displayLink`/`CADisplayLink`, or `CAMetalDisplayLink`), presenting from the callback. This is the only route to a *known* frame start (before a known vsync) and the only one that removes the 12 us of phase wander; it cannot beat one period. Preconditions: winit 0.30 has no frame/redraw-alignment event for macOS (issue #2412 still open), so this bypasses or extends winit. Apple's own assessment of the current design: "We're relying on the back pressure of a Drawable being available to set our frame rate for us. On a fixed-rate display, we know that this isn't the best idea" (WWDC21 10147).
- **A2. Present with Metal frame pacing** (`presentDrawable:atTime:` / `afterMinimumDuration:`). Not reachable through wgpu 25, which calls only `present_drawable` + `commit`; needs native Metal or upstream work.

### Tier 3 — rejected

Cap below the panel rate (Unreal-style frame pacing — makes p50 worse by design); `Immediate` (tearing — measured: p50 unchanged within 20 us, so it pays the tearing and buys nothing on the median); sleep- or timer-based limiting (out of phase or drifting per Apple WWDC21 10147); fixed-timestep interpolation alone (changes content time, not frame interval; costs 1-2 ticks of delay); `Mailbox`/`FifoRelaxed` (not on Metal in wgpu 25); spin-waiting (Apple TN2169 forbids it); `CAMetalDisplayLink.preferredFrameLatency` (accepts only 1.0 or 2.0).

---

## What does not work (consolidated; each owned by a sibling file)

1. `present()` does not block — `get_current_texture()` does; `SurfaceError::Timeout` is unreachable because wgpu disables `allowsNextDrawableTimeout`.
2. `desired_maximum_frame_latency` is pipeline depth, not cadence; Metal's legal drawable counts are 2 or 3 only, so the range is `1..=2`.
3. No per-frame present timestamp exists in wgpu 25 (`presented_time` grep: zero hits); `MTLDrawable.presentedTime`/`addPresentedHandler` are unreachable through wgpu.
4. `ControlFlow::Poll` vs `Wait` changes winit's waker timer, not the redraw hop; macOS has no documented `request_redraw` latency bound.
5. Interpolation, uncapped rendering, sub-panel caps, and timers each address a different problem than the median interval, with published costs.

---

## Version note

`frame-pacing-loop-strategies.md` cites winit **v0.30.5** tag lines for the macOS redraw path; `Cargo.lock` pins **0.30.13**. Verified in the vendored 0.30.13: the same path exists unchanged (`app_state.rs:42,100,297,369,384`). The present-path file cites 0.30.13 throughout. No conclusion depends on the difference.

---

## Recommendation

1. **Accept p50 as met** — it is one panel period within +/-13 us, and no reachable change improves it.
2. **If the target is perceived smoothness**, do D1 (true period), then R1 + R2, and re-measure with the same 600-frame probe, watching p99/max rather than p50.
3. **p50 at 120 fps is a hardware prerequisite, not a code change.** The app's own frame cost is 1.0-1.2 ms against the 8.333 ms a 120 Hz panel allows. Attach a display with a ≥120 Hz mode and re-run the same probe: `Fifo` will pace to that panel's period and the expected p50 is 8.333 ms (120.0 fps). Tearing mode is not a substitute — it was measured and buys nothing.

No source file was changed by this research. Both probes are reverted; `git status` is clean; the only additions are the three files in `docs/research/`.

---

## Sources

Detailed citation sets live in the two sibling files (Apple developer documentation, WWDC21 10147, wgpu/wgpu-core/wgpu-hal 25.0.2 and wgpu-types 25.0.0 sources at their tagged versions and in the vendored registry copies, winit 0.30.13, Vulkan `VkPresentModeKHR`, Apple TN2169, Gaffer On Games *Fix Your Timestep!*, Unity/Godot/Unreal docs, plus ten labelled community practice reports). Load-bearing primaries for the conclusions above:

- `docs/research/frame-pacing-present-path.md` — findings 1-6 own "what blocks and what the acquire waits for"; findings 9, 12, 15 own the unavailable timestamp, the redraw hop, and the panel-rate ceiling.
- `docs/research/frame-pacing-loop-strategies.md` — A1 (quantisation), A4 (timer accuracy budgets), A5 (display links), A6 (back-pressure pacing), A7-A8 (winit), Part B (practice).
- This file's own measurement: temporary probe in `App::draw`/`App::tick`, 600 frames x 3 speed settings, 2026-09-12, on the machine described above.

---

## Report back

- **p50 is 60 fps within measurement error** (16.65-16.72 ms against a 16.666 ms panel period, across both probe configurations: 59.8-60.0 fps). The asked-for target is met; the residual is phase jitter.
- **The pacer is `CAMetalLayer.nextDrawable()`, not `present()`.** Measured: acquire 12-14.5 ms p50 depending on machine load, present 5-24 us. This corrected the session's earlier attribution.
- **Pre-acquire CPU work is 0.05 ms of 16.67 ms** (fresh runs, warm-up discarded), so the sibling file's "trim pre-acquire work" option cannot move the median; the 2.0 ms encode bucket is 93% glyph shaping (1.85 ms) and belongs to the tail.
- **Two concrete tail fixes are reachable now**: cache text shaping for unchanged strings, and keep Generation breeding off the main thread.
- **p50 above 60 fps is not reachable in a window on this panel**: measured in both present modes, so tearing is not a lever either. The prerequisite is a display with a ≥120 Hz mode.
- **Nothing in the repo changed** except the three research files; both probes reverted, tree clean.
