# Frame Pacing on the macOS Present Path — What Decides the Interval Between Two `present()` Calls (wgpu 25 / winit 0.30 / Metal)

**Date:** 2026-09-12
**Question:** In `neuroarena` (wgpu 25.0.2 + winit 0.30.13 + Metal, macOS, 60.00 Hz panel), what actually decides the wall-clock interval between two `present()` calls, and which knobs exist at the pinned versions to bring the median interval to `<= 16.667 ms`?

**Pinned versions (from `Cargo.lock`):** `wgpu 25.0.2`, `wgpu-core 25.0.2`, `wgpu-hal 25.0.2`, `wgpu-types 25.0.0`, `winit 0.30.13`, `metal 0.31.0`.

**Method.** Every mechanism claim is traced to the source that owns it. Crate claims were read from the vendored sources at `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/<crate>-<version>/` — the exact code this binary links — with the equivalent tagged GitHub URL cited. Apple claims are quoted from `developer.apple.com` symbol pages. Tags: **[PRIMARY]** = owning source; **[ANECDOTE]** = community/forum report about practice; **[INFERENCE]** = my own reasoning from the primaries.

**Curated-corpus pass (context-awesome), negative result.** `find_awesome_section` and `search_awesome_items` over the curated corpus returned no macOS / Metal / wgpu / game-loop / frame-pacing list. The only hit was `awesomelistsio/awesome-web-performance` → section "Performance Metrics" (2 items, web-only). Nothing from the corpus is cited below; no item contributed to any finding. Everything here is Apple, wgpu, winit or Khronos/WebGPU primary material.

---

## The question

The measured starting point (author's machine: Apple M5, built-in Liquid Retina 1710×1112 @ 60.0 Hz per `CGDisplayCopyDisplayMode`, `NSScreen.maximumFramesPerSecond` = 60; 600 frames per run):

| speed | mean | p50 | p95 | p99 | max | sim | draw+present |
|---|---|---|---|---|---|---|---|
| x0 | 16.602 ms (60.23 fps) | **16.658** | 17.084 | 17.320 | 19.640 | 0.001 ms | 16.544 ms |
| x100 (default) | 16.630 ms (60.13 fps) | **16.679** | 17.132 | 18.377 | 40.182 | 0.123 ms | 16.428 ms |
| unbounded | 16.625 ms (60.15 fps) | **16.664** | 17.122 | 17.435 | 32.528 | 1.579 ms | 14.974 ms |

One panel period at 60.00 Hz is **16.6667 ms**. The gap at the default speed is therefore **~12 microseconds on the median**, and the `x0` run already sits *below* the target. The `draw+present` column — 16.5 ms out of a 16.6 ms frame — says the frame interval is consumed almost entirely by whatever `present`/acquire does, not by CPU rendering. So the question is not "how do we go faster" but "what is the interval actually quantised against, and is `16.6667` even the right number".

That question has a precise answer in the pinned code, and it is not the one the API names suggest: **`present()` is non-blocking; `get_current_texture()` is where the frame interval is spent, and it blocks on `CAMetalLayer.nextDrawable()`, which Apple says returns "usually at the display's next refresh interval".**

---

## Findings

### 1. `present()` does not block. The next frame's acquire blocks. [PRIMARY]

`SurfaceTexture::present` does nothing but flag and dispatch:

```rust
pub fn present(mut self) {
    self.presented = true;
    self.detail.present();
}
```

- **Source:** [wgpu 25.0.2 `SurfaceTexture::present`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu/src/api/surface_texture.rs) — `wgpu/src/api/surface_texture.rs`, `impl SurfaceTexture`, `pub fn present(mut self) -> ()` (vendored: `wgpu-25.0.2/src/api/surface_texture.rs:37-40`).

The Metal backend's `present` commits one command buffer and returns `Ok(())`:

```rust
let command_buffer = queue.new_command_buffer();
command_buffer.set_label("(wgpu internal) Present");
if !texture.present_with_transaction {
    command_buffer.present_drawable(&texture.drawable);
}
command_buffer.commit();
```

- **Source:** [wgpu-hal 25.0.2 Metal `Queue::present`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/mod.rs) — `wgpu-hal/src/metal/mod.rs`, `fn present(...)`, lines 468-492 (vendored: `wgpu-hal-25.0.2/src/metal/mod.rs:468-492`).

So the app's own comment in `app/src/appstate.rs` is correct in the only sense that matters — a dropped frame leaves a blank window — but `frame.present()` is not a pacing point. Note also that this path allocates and commits **a second `MTLCommandBuffer` per frame** (in addition to the render command buffer), labelled `"(wgpu internal) Present"`; with `presents_with_transaction == false` (hard-coded in wgpu 25, see finding 12), that buffer carries only the drawable presentation.

### 2. The blocking call is `get_current_texture()` → `CAMetalLayer.nextDrawable()`, and what it waits for is a free drawable. [PRIMARY]

wgpu's own `PresentMode::Fifo` documentation states the contract in terms of the acquire, not the present:

> "Presentation frames are kept in a First-In-First-Out queue approximately 3 frames long. Every vertical blanking period, the presentation engine will pop a frame off the queue to display. If there is no frame to display, it will present the same frame again until the next vblank. When a present command is executed on the GPU, the presented image is added on the queue. **Calls to `Surface::get_current_texture()` will block until there is a spot in the queue.**"
> — `* **Tearing:** No tearing will be observed.` / `* **Supported on**: All platforms.` / `* **Also known as**: "Vsync On"`

- **Source:** [wgpu-types 25.0.0 `PresentMode::Fifo`](https://docs.rs/wgpu-types/25.0.0/wgpu_types/enum.PresentMode.html) — `wgpu-types-25.0.0/src/lib.rs`, `enum PresentMode`, variant `Fifo` (vendored: `wgpu-types-25.0.0/src/lib.rs:5159-5176`).

The Metal implementation behind that doc is one call:

```rust
pub fn next_drawable(&self) -> Option<&MetalDrawableRef> {
    unsafe { msg_send![self, nextDrawable] }
}
```

- **Source:** [metal 0.31.0 `MetalLayerRef::next_drawable`](https://docs.rs/metal/0.31.0/metal/struct.MetalLayer.html) — `metal-0.31.0/src/lib.rs`, `impl MetalLayerRef`, `next_drawable()` (vendored: `metal-0.31.0/src/lib.rs:504-506`); called from `wgpu-hal/src/metal/surface.rs`, `fn acquire_texture`, line 195.

Apple states exactly what that waits for:

> "Each drawable comes from a limited and reusable resource pool. A drawable may not always be available when your app requests one. When that happens, **`nextDrawable()` blocks the calling thread until a drawable becomes available — usually at the display's next refresh interval**."

- **Source:** [Apple — Onscreen presentation](https://developer.apple.com/documentation/metal/onscreen-presentation) — `Onscreen presentation`, Overview.

And what makes a drawable available again:

> "The layer reuses a drawable only if it isn't onscreen and there are no strong references to it."
> "If you don't release drawables correctly, the layer runs out of drawables, and future calls to `nextDrawable()` return `nil`."

- **Source:** [Apple — `CAMetalLayer`](https://developer.apple.com/documentation/quartzcore/cametallayer) — `CAMetalLayer`, section "Keeping References to Drawables".

`nextDrawable()`'s own page states the same in the API's terms: **"Waits until a Metal drawable is available, and then returns it."** with the discussion "If all drawables are in use, the layer waits up to one second for one to become available, after which it returns `nil`" — the one-second bound is the *default* and is defeated by wgpu (finding 5).

- **Source:** [Apple — `CAMetalLayer.nextDrawable()`](https://developer.apple.com/documentation/quartzcore/cametallayer/nextdrawable()).

**Consequence for the measurement:** the interval between two `tick()` calls is dominated by two consecutive `nextDrawable()` returns. That is why `draw+present` reads 16.4-16.5 ms and `sim` reads 0.001-1.6 ms.

### 3. `PresentMode::Fifo` on Metal is exactly one `CAMetalLayer` property: `displaySyncEnabled`. [PRIMARY]

```rust
let display_sync = match config.present_mode {
    wgt::PresentMode::Fifo => true,
    wgt::PresentMode::Immediate => false,
    m => unreachable!("Unsupported present mode: {m:?}"),
};
...
if caps.can_set_display_sync {
    let () = msg_send![*render_layer, setDisplaySyncEnabled: display_sync];
}
```

- **Source:** [wgpu-hal 25.0.2 Metal `Surface::configure`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/surface.rs) — `wgpu-hal/src/metal/surface.rs`, lines 145-179 (vendored: `wgpu-hal-25.0.2/src/metal/surface.rs:145-179`). `can_set_display_sync` is `version.at_least((10, 13), OS_NOT_SUPPORT, os_is_mac)` (`metal/adapter.rs:847`), so on any supported macOS it is enabled.

Apple defines the property as the vsync switch:

> "Set this value to `true` to synchronize the presentation of the layer's contents with the display's refresh, also known as [vsync or v-sync]. If `false`, the layer presents new content more quickly, but possibly with brief visual artifacts ([tearing]). The default value is `true`."

- **Source:** [Apple — `CAMetalLayer.displaySyncEnabled`](https://developer.apple.com/documentation/quartzcore/cametallayer/displaysyncenabled) (macOS 10.13+).

`PresentMode::FifoRelaxed`, `Mailbox` and `AutoVsync`/`AutoNoVsync` never reach this `match`: the Metal backend's `unreachable!` arms them, and the surface capabilities advertised on Metal are only `[Fifo, Immediate]` (or `[Fifo]` where `setDisplaySyncEnabled:` is unavailable).

- **Source:** [wgpu-hal 25.0.2 Metal `surface_capabilities`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/adapter.rs) — `present_modes: if pc.can_set_display_sync { vec![wgpu_types::PresentMode::Fifo, wgpu_types::PresentMode::Immediate] } else { vec![wgpu_types::PresentMode::Fifo] }` (vendored: `wgpu-hal-25.0.2/src/metal/adapter.rs:369-373`).

### 4. `desired_maximum_frame_latency` on Metal is literally `maximumDrawableCount = latency + 1`. [PRIMARY]

```rust
// this gets ignored on iOS for certain OS/device combinations (iphone5s iOS 10.3)
render_layer.set_maximum_drawable_count(config.maximum_frame_latency as u64 + 1);
```

- **Source:** [wgpu-hal 25.0.2 Metal `Surface::configure`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/surface.rs) — line 171 (vendored: `wgpu-hal-25.0.2/src/metal/surface.rs:171`). The capability range carries the comment "We use this here to govern the maximum number of drawables + 1. See https://developer.apple.com/documentation/quartzcore/cametallayer/2938720-maximumdrawablecount" — [wgpu-hal 25.0.2 Metal `surface_capabilities`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/adapter.rs), lines 358-367.

So at the app's current setting (`present_mode: Fifo`, `desired_maximum_frame_latency: 2` in `app/src/appstate.rs:753-754`):

| `desired_maximum_frame_latency` | `maximumDrawableCount` | wgpu doc description |
|---|---|---|
| 1 | 2 | "low latency from frame recording to frame display" |
| 2 (default, and this app) | 3 | "potentially smoother frame display, as it allows to be at least one frame to be queued up" |
| 0, 3, 4, … | — | clamped to 1 or 2 before reaching the layer |

The exact documented meaning of the field:

> "Desired maximum number of frames that the presentation engine should queue in advance. This is a hint to the backend implementation and will always be clamped to the supported range. As a consequence, either the maximum frame latency is set directly on the swap chain, or waits on present are scheduled to avoid exceeding the maximum frame latency if supported, or the swap chain size is set to (max-latency + 1). Defaults to 2 when created via `Surface::get_default_config`. … Choose 1 for low latency from frame recording to frame display. ⚠️ If the backend does not support waiting on present, this will cause the CPU to wait for the GPU to finish all work related to the previous frame when calling `Surface::get_current_texture`, causing CPU-GPU serialization … It is currently not possible to query this. See https://github.com/gfx-rs/wgpu/issues/2869. A value of 0 is generally not supported and always clamped to a higher value."

- **Source:** [wgpu-types 25.0.0 `SurfaceConfiguration::desired_maximum_frame_latency`](https://docs.rs/wgpu-types/25.0.0/wgpu_types/struct.SurfaceConfiguration.html) — `wgpu-types-25.0.0/src/lib.rs:5445-5455` (vendored). `Surface::get_default_config` sets `desired_maximum_frame_latency: 2` — [wgpu 25.0.2 `Surface::get_default_config`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu/src/api/surface.rs) (vendored: `wgpu-25.0.2/src/api/surface.rs:70`).

The C-side of the same API defines the field more sharply, in units of the measurement at hand:

> "Desired maximum number of frames in flight (i.e. **the number of monitor refreshes between `wgpuSurfaceGetCurrentTexture` and presentation**). - 1: Minimize latency (CPU and GPU cannot run in parallel). - 2: Balance between latency and throughput (the default). - 3+: Maximize throughput."

- **Source:** [wgpu-native `ffi/wgpu.h`](https://github.com/gfx-rs/wgpu-native/blob/trunk/ffi/wgpu.h) — `typedef struct WGPUSurfaceConfigurationExtras`, member `desiredMaximumFrameLatency` (fetched 2026-09-12).

The clamp is applied by wgpu-core before the backend sees the value:

```rust
let maximum_frame_latency = config.desired_maximum_frame_latency.clamp(
    *caps.maximum_frame_latency.start(),
    *caps.maximum_frame_latency.end(),
);
```

- **Source:** [wgpu-core 25.0.2 `surface_configure`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-core/src/device/global.rs) — `wgpu-core/src/device/global.rs:1845-1848`, with the Metal range `1..=2` from `metal/adapter.rs:362-367`.

**This is the crux for the p50 question** [INFERENCE, from findings 1-4 and 6]: the knob sets **pipeline depth (how many refreshes separate acquire from display)**, not **cadence**. In a saturated Fifo loop the cadence is one presented frame per refresh regardless of the depth; changing 2 → 1 changes the frame's phase relative to the display (input-to-photon latency) and how often the acquire blocks, not the number of frames the display consumes per second. Any expected p50 improvement from this knob must therefore come from the *distribution* (fewer drift-ahead short intervals, more blocking), not from a shift of the cadence itself.

### 5. On macOS 10.13+, wgpu disables `allowsNextDrawableTimeout`, so the acquire waits **indefinitely**, and wgpu's own 1 s acquire timeout never fires. [PRIMARY]

```rust
if caps.can_set_next_drawable_timeout {
    let () = msg_send![*render_layer, setAllowsNextDrawableTimeout:false];
}
```

- **Source:** [wgpu-hal 25.0.2 Metal `Surface::configure`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/surface.rs) — lines 173-175; capability gate `can_set_next_drawable_timeout: version.at_least((10, 13), (11, 0), os_is_mac)` at `metal/adapter.rs:848`.

Apple: "If `true`, the `nextDrawable()` method returns `nil` if it can't provide a drawable object within one second. **If `false`, the `nextDrawable()` method waits indefinitely for a drawable to become available.** The default value is `true`."
— [Apple — `CAMetalLayer.allowsNextDrawableTimeout`](https://developer.apple.com/documentation/quartzcore/cametallayer/allowsnextdrawabletimeout) (macOS 10.13+).

Meanwhile wgpu-core *does* pass a timeout, and the Metal backend *discards* it:

```rust
const FRAME_TIMEOUT_MS: u32 = 1000;
...
suf.acquire_texture(Some(core::time::Duration::from_millis(FRAME_TIMEOUT_MS as u64)), fence.as_ref())
```
```rust
unsafe fn acquire_texture(
    &self,
    _timeout_ms: Option<std::time::Duration>, //TODO
    _fence: &super::Fence,
) -> Result<Option<crate::AcquiredSurfaceTexture<super::Api>>, crate::SurfaceError> {
```

- **Sources:** [wgpu-core 25.0.2 `present.rs`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-core/src/present.rs) — `const FRAME_TIMEOUT_MS: u32 = 1000;` (line 29) and the `acquire_texture(Some(...))` call site (lines 133-137); [wgpu-hal 25.0.2 Metal `Surface::acquire_texture`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/surface.rs) — lines 186-198.

The plumbing is worth stating exactly, because it is easy to infer the wrong thing: wgpu-core maps a HAL `Ok(None)` to `(None, Status::Timeout)` (`wgpu-core/src/present.rs:217`) and wgpu maps that status to `SurfaceError::Timeout` (`wgpu/src/api/surface.rs`, `SurfaceStatus::Timeout => return Err(SurfaceError::Timeout)`). So the `Timeout` variant *is* wired up on the Metal path — but the only way to reach it is for `next_drawable()` to return `nil`, and wgpu has turned off the property that makes it do so (default `true`: "returns `nil` if it can't provide a drawable object within one second"). Consequently the 1000 ms value in `FRAME_TIMEOUT_MS` is never enforced by this backend: for ordinary back-pressure the call blocks past any deadline rather than reporting a timeout.

*Practical reading:* if the drawable pool is ever exhausted for a reason other than ordinary vsync back-pressure, this app's main thread blocks forever inside `get_current_texture()`. That is a robustness fact, not a pacing one, but it is worth knowing before experimenting with `desired_maximum_frame_latency = 1` and lower drawable counts.

### 6. `nextDrawable()` returns when a drawable is *released*, and "the display's next refresh interval" is Apple's own hedge — not a guarantee of exactly one period. [PRIMARY]

The owning sentence is finding 2's quote — **"usually at the display's next refresh interval"** ([Apple — Onscreen presentation](https://developer.apple.com/documentation/metal/onscreen-presentation)) — combined with "The layer reuses a drawable only if it isn't onscreen and there are no strong references to it" ([`CAMetalLayer`](https://developer.apple.com/documentation/quartzcore/cametallayer), "Keeping References to Drawables"). Both sentences describe the pool, not a clock: a drawable comes back when Core Animation is done with it, and the layer's docs never state that this event is aligned to a vblank boundary.

**Negative result:** Apple publishes **no per-frame compositor-latency figure and no statement that a windowed `CAMetalLayer` present lands at the exact panel period**. The closest owning statements are the two above, plus the asynchrony note in `presentsWithTransaction` (finding 12). Any claim that "windowed Metal presents at exactly the panel period on macOS" is not backed by an Apple document I could find.

### 7. The present itself is *scheduled*, not executed: `presentDrawable` installs a scheduled handler that calls `drawable.present()`. [PRIMARY]

Apple documents `MTLCommandBuffer.present(_:)` (the Swift spelling of `presentDrawable:`) as a delegation:

> "This convenience method calls the drawable's `present()` method **after the command queue schedules the command buffer for execution**. The command buffer does this by adding a completion handler by calling its own `addScheduledHandler(_:)` method for you."

- **Source:** [Apple — `MTLCommandBuffer.present(_:)`](https://developer.apple.com/documentation/metal/mtlcommandbuffer/present(_:)).

And the drawable side:

> "When a command queue schedules a command buffer for execution, it tracks whether any commands in that command buffer need to render or write to the drawable object. When you call this method, the drawable presents its contents as soon as possible after all scheduled render or write requests for that drawable are complete."

- **Source:** [Apple — `MTLDrawable.present()`](https://developer.apple.com/documentation/metal/mtldrawable/present()) — "Presents the drawable onscreen as soon as possible."

Both are *as soon as possible*, not *at a stated time*. The scheduled variant does exist — `presentDrawable:atTime:`, i.e. `MTLDrawable.present(at:)` ("Presents the drawable onscreen at a specific host time… The Mach absolute time at which the drawable should be presented, in seconds", macOS 10.11+) — but wgpu 25 never calls it, and the `metal` 0.31.0 crate does not even bind it: the only presentation entry point there is `CommandBufferRef::present_drawable` (`metal-0.31.0/src/commandbuffer.rs:88`).

- **Sources:** [Apple — `MTLDrawable.present(at:)`](https://developer.apple.com/documentation/metal/mtldrawable/present(at:)); [metal 0.31.0 `commandbuffer.rs`](https://docs.rs/metal/0.31.0/metal/struct.CommandBuffer.html).

`MTLPresentMode` — the enum the brief asks about — **is not public API and not reachable**: `/documentation/Metal/MTLPresentMode` returns HTTP 404 from Apple's documentation service (probed 2026-09-12), and neither the `metal` crate nor wgpu-hal references it. The reachable present-path controls are exactly: `presentDrawable` (used), `presentDrawable:atTime:` (unbound), `CAMetalLayer.displaySyncEnabled` (set from `PresentMode`), `CAMetalLayer.presentsWithTransaction` (hard-coded `false`, finding 12), `CAMetalLayer.maximumDrawableCount` (set from `desired_maximum_frame_latency`) and `CAMetalLayer.allowsNextDrawableTimeout` (forced `false`).

### 8. `maximumDrawableCount` accepts only 2 or 3 — the legal Metal drawable depths are exactly the two values wgpu's clamp allows. [PRIMARY]

> "The number of Metal drawables in the resource pool managed by Core Animation. … **You can set this value to 2 or 3 only; if you pass a different value, Core Animation ignores the value and throws an exception. The default value is 3.**"

- **Source:** [Apple — `CAMetalLayer.maximumDrawableCount`](https://developer.apple.com/documentation/quartzcore/cametallayer/maximumdrawablecount) (macOS 10.13.2+).

wgpu's Metal path is a **correct** mapping rather than a coincidence: wgpu's capability range `1..=2` (latency) maps onto Apple's `2 or 3` (drawables), and wgpu-core clamps into it. The same field is *not* clamped per-backend elsewhere: DX12 advertises `1..=16` and applies `SetMaximumFrameLatency` with `(maximum_frame_latency + 1).min(16)` swap-chain buffers (`wgpu-hal-25.0.2/src/dx12/adapter.rs:884`, `dx12/mod.rs:1183-1190`), and Vulkan derives the range from the swap-chain image count and folds the value into `min_image_count(config.maximum_frame_latency + 1)` with a `// TODO: https://github.com/gfx-rs/wgpu/issues/2869` (`vulkan/adapter.rs:2508`, `vulkan/device.rs:575`). **The Vulkan-only caveat in the brief does not exist at this version in the form "the field is honoured on Vulkan and ignored elsewhere" — the reverse is true: on Metal wgpu implements it directly, on Vulkan it only sizes the swap chain, and the wait-on-present strategy is the unimplemented one.**

### 9. wgpu 25 exposes no per-frame present timestamp. This is a verified absence with named owners. [PRIMARY for the absence]

Grepping `wgpu-25.0.2/src`, `wgpu-hal-25.0.2/src`, `wgpu-core-25.0.2/src` and `wgpu-types-25.0.0/src` for `presented_time`, `PresentedTime`, `presented_handler` returns **zero hits**. `SurfaceTexture::present` returns `()` and the struct's public fields are `texture` and `suboptimal` only ([`wgpu/src/api/surface_texture.rs`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu/src/api/surface_texture.rs), lines 11-21 and 27-40, vendored). Nothing in the 25.0.2 API returns the `MTLDrawable` that `wgpu-hal`'s private `SurfaceTexture` retains (`wgpu-hal-25.0.2/src/metal/mod.rs:392-396`: `drawable: metal::MetalDrawable`).

The Metal primitives that *would* provide it exist in the `metal` crate and are simply unused: `DrawableRef::add_presented_handler` and `DrawableRef::presented_time` ([metal 0.31.0 `drawable.rs`](https://docs.rs/metal/0.31.0/metal/struct.Drawable.html), lines 32-38).

The owning wgpu issues:

- [gfx-rs/wgpu#2869 — "Extended Presentation API Investigation"](https://github.com/gfx-rs/wgpu/issues/2869) (open, labels `type: enhancement`, `area: wsi`, `type: tracking`, `backend: metal`) is the tracking issue; `wgpu-types`' own doc for `desired_maximum_frame_latency` links to it for "It is currently not possible to query this". Its WSI table records for `CAMetalLayer`: *Present Time* ✅ "(1c) Presentation times are given through callbacks", *Wait for Present* ✅, *Present with Damage* ❌, *Scheduled Present* ✅, *Monitor Frequency* ✅ "(5) Via NSScreen". The proposed `MonitorStatistics` struct is where a `min_refresh_interval` / `max_refresh_interval` / CAMetalLayer `display_update_granularity` would land.
- [gfx-rs/wgpu#9856 — "Metal-first frame-specific presentation feedback for SurfaceTexture"](https://github.com/gfx-rs/wgpu/issues/9856) (open, filed 2026-07-12) records the absence against a **newer** version than this repo pins: "On wgpu 29.0.4, `SurfaceTexture::present(self)` returns `()` and the public `SurfaceTexture` exposes no token that can later be paired with actual presentation." The same body states the consequence — "GPU completion and return from `present()` are both earlier than display presentation" — and proposes `Queue::present_with_feedback(SurfaceTexture) -> PresentationFeedbackFuture` implemented by installing `MTLDrawable.addPresentedHandler` before `presentDrawable`, reporting `presentedTime == 0` as not-presented. No maintainer has accepted an API shape; comment 2 (2026-08-14) still lists four open design questions.

**So: no, wgpu 25 — and no released wgpu as of 2026-09 — lets an application see when its frame was actually displayed.** What Apple *does* offer for this, and what it is for:

> "**The host time, in seconds, when the drawable was displayed onscreen.** … The property value is `0` if the drawable hasn't been presented or if its associated frame was dropped."

- **Source:** [Apple — `MTLDrawable.presentedTime`](https://developer.apple.com/documentation/metal/mtldrawable/presentedtime) (macOS 10.15.4+).

> "Registers a block of code to be called immediately after the drawable is presented. … You can register multiple handlers for a single drawable object. The following example code schedules a presentation handler that **reads the `presentedTime` property and uses it to derive the interval between the last and current presentation times. From that information, it determines the app's frame rate.**"

- **Source:** [Apple — `MTLDrawable.addPresentedHandler(_:)`](https://developer.apple.com/documentation/metal/mtldrawable/addpresentedhandler(_:)) (macOS 10.15.4+).

That second quote is Apple's own answer to "how do I measure frame interval on macOS": from `presentedTime`, not from CPU timestamps around `present()`. [ANECDOTE] For practice: the #9856 author reports a 120 Hz real-window calibration where "the time from `SurfaceTexture::present()` return to the same `CADisplayLink.targetTimestamp` was p50 7.613 ms / p95 7.858 ms over 28 valid samples" and notes that this fallback "cannot prove that the particular drawable was shown".

### 10. winit 0.30.13's macOS `request_redraw` does **not** call `setNeedsDisplay`. It enqueues and wakes the run loop. [PRIMARY]

```rust
pub fn request_redraw(&self) {
    self.ivars().app_delegate.queue_redraw(self.window().id());
}
```
```rust
pub fn queue_redraw(&self, window_id: WindowId) {
    let mut pending_redraw = self.ivars().pending_redraw.borrow_mut();
    if !pending_redraw.contains(&window_id) {
        pending_redraw.push(window_id);
    }
    self.ivars().run_loop.wakeup();
}
```
```rust
pub fn wakeup(&self) {
    unsafe { CFRunLoopWakeUp(self.0) }
}
```

- **Sources:** [winit 0.30.13 `window_delegate.rs`](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/window_delegate.rs) — `WindowDelegate::request_redraw`, line 916; [winit 0.30.13 `app_state.rs`](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/app_state.rs) — `queue_redraw`, lines 297-303; [winit 0.30.13 `observer.rs`](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/observer.rs) — `RunLoop::wakeup`, lines 111-113. (Vendored at `winit-0.30.13/src/platform_impl/macos/`.)

`CFRunLoopWakeUp(_:)` is documented as "Wakes a waiting `CFRunLoop` object." ([Apple — `CFRunLoopWakeUp(_:)`](https://developer.apple.com/documentation/corefoundation/cfrunloopwakeup(_:))). There is no `setNeedsDisplay` anywhere on this path: winit's macOS redraw is a private queue plus a run-loop wakeup. (winit's view does implement `-[NSView drawRect:]` at `view.rs:202-208`, which calls `handle_redraw` synchronously — but that path is reached only when *AppKit* decides the view needs display, not from `request_redraw`, and winit's own view never sets a `CAMetalLayer` or calls `setNeedsDisplay`.)

### 11. Where `RedrawRequested` is delivered: in the `kCFRunLoopBeforeWaiting` observer, immediately before `AboutToWait` — so `about_to_wait`'s request lands in the **next** iteration. [PRIMARY]

winit installs exactly two run-loop observers (`setup_control_flow_observers`):

```rust
run_loop.add_observer(kCFRunLoopAfterWaiting, CFIndex::MIN, control_flow_begin_handler, ...);
run_loop.add_observer(kCFRunLoopExit | kCFRunLoopBeforeWaiting, CFIndex::MAX, control_flow_end_handler, ...);
```

`kCFRunLoopAfterWaiting` → `ApplicationDelegate::wakeup(panic_info)` (dispatches `NewEvents`); `kCFRunLoopBeforeWaiting` → `ApplicationDelegate::cleared(panic_info)`. And `cleared()` is, in order:

```rust
self.handle_event(Event::UserEvent(HandlePendingUserEvents));

let redraw = mem::take(&mut *self.ivars().pending_redraw.borrow_mut());
for window_id in redraw {
    self.handle_event(Event::WindowEvent { window_id: RootWindowId(window_id), event: WindowEvent::RedrawRequested });
}

self.handle_event(Event::AboutToWait);
...
self.ivars().waker.borrow_mut().start_at(min_timeout(wait_timeout, app_timeout));
```

- **Sources:** [winit 0.30.13 `observer.rs`](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/observer.rs) — `setup_control_flow_observers` (lines 206-231), `control_flow_begin_handler` (lines 51-69), `control_flow_end_handler` (lines 73-92); [winit 0.30.13 `app_state.rs`](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/app_state.rs) — `ApplicationDelegate::cleared`, lines 373-420.

Therefore one `tick()` costs **one full CFRunLoop iteration**: `RedrawRequested` → app `tick()` (which blocks in `get_current_texture()`) → `AboutToWait` → app `request_redraw()` → `queue_redraw` (push + `CFRunLoopWakeUp`) → run loop waits → next iteration. The app's `about_to_wait` (`app/src/appstate.rs:809-813`) cannot produce a redraw in the same iteration; the event is drained at the *next* `kCFRunLoopBeforeWaiting`.

### 12. `ControlFlow::Poll` vs `Wait` changes the waker timer, not the redraw hop — but the app never sets either, so it runs on `Wait` and is kept alive by `CFRunLoopWakeUp`. [PRIMARY + INFERENCE]

`cleared()` ends by arming winit's waker timer from the control flow:

```rust
let app_timeout = match self.control_flow() {
    ControlFlow::Wait => None,
    ControlFlow::Poll => Some(Instant::now()),
    ControlFlow::WaitUntil(instant) => Some(instant),
};
self.ivars().waker.borrow_mut().start_at(min_timeout(wait_timeout, app_timeout));
```

and `start_at` maps `Some(t)` with `now >= t` to `start()` → `CFRunLoopTimerSetNextFireDate(self.timer, f64::MIN)`, while `None` maps to `stop()` → `CFRunLoopTimerSetNextFireDate(self.timer, f64::MAX)`. The timer itself is created as `CFRunLoopTimerCreate(..., 0.000_000_1, ...)` — "Create a timer with a 0.1µs interval (1ns does not work) to mimic polling."

- **Sources:** [winit 0.30.13 `app_state.rs`](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/app_state.rs) — lines 406-410; [winit 0.30.13 `observer.rs`](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/observer.rs) — `EventLoopWaker::new` (lines 236-253), `start` (281-286), `start_at` (288-306). The app's `control_flow` cell is initialised to `ControlFlow::default()` (`app_state.rs:96`), and winit documents `ControlFlow` as "Indicates the desired behavior of the event loop after `Event::AboutToWait` is emitted. **Defaults to `Wait`**" ([winit 0.30.13 `ControlFlow`](https://docs.rs/winit/0.30.13/winit/event_loop/enum.ControlFlow.html)).

`neuroarena` never calls `set_control_flow` (`app/src/appstate.rs` has no such call; `about_to_wait` only calls `request_redraw`), so it runs the `Wait` branch: the waker timer is **disarmed** every iteration. The loop nevertheless turns because `queue_redraw()` calls `CFRunLoopWakeUp` immediately before the run loop waits. [INFERENCE] Whether `CFRunLoopWakeUp` issued during the `kCFRunLoopBeforeWaiting` observer still prevents the subsequent sleep is not documented by Apple beyond "Wakes a waiting CFRunLoop object"; the observed ~60 fps over 600 frames in the measurement is the empirical evidence that it does, and the source shows it is the *only* thing that could.

**Practical answer to the brief's question:** `ControlFlow::Poll` would arm an immediately-firing timer and thus remove any dependence on `CFRunLoopWakeUp` semantics; `ControlFlow::Wait` (the current state) relies on it. Neither changes *where* in the iteration `RedrawRequested` is delivered, so neither changes the structure of the frame's critical path — the block in `get_current_texture()` still dominates. The hop itself is bounded by one run-loop iteration plus the `CFRunLoopWakeUp` turnaround.

winit documents **no latency guarantee** for this hop, and macOS is conspicuously absent from the platform-specific list:

> "There are no strong guarantees about when exactly a `RedrawRequest` event will be emitted with respect to other events, since the requirements can vary significantly between windowing systems. However as the event aligns with the windowing system drawing loop, it may not arrive in same or even next event loop iteration."
> "## Platform-specific — **Windows** This API uses `RedrawWindow` … — **iOS:** Can only be called on the main thread. — **Wayland:** The events are aligned with the frame callbacks when `Window::pre_present_notify` is used. — **Web:** `WindowEvent::RedrawRequested` will be aligned with the `requestAnimationFrame`."

- **Source:** [winit 0.30.13 `Window::request_redraw`](https://github.com/rust-windowing/winit/blob/v0.30.13/src/window.rs) — doc comment, lines 572-596 (vendored: `winit-0.30.13/src/window.rs:572-596`). Note the contrast with Wayland/Web: on those platforms winit aligns redraws with the compositor's frame callbacks; on macOS it does not, because there is no such alignment in this implementation.

### 13. The drawable this app presents into is a `CAMetalLayer` that **wgpu** created, as a *sublayer* of the winit view's layer. [PRIMARY]

winit's macOS view sets no `CAMetalLayer` (`grep` for `CAMetalLayer`, `wantsLayer`, `makeBackingLayer` over `winit-0.30.13/src/platform_impl/macos/view.rs` returns nothing). wgpu-hal therefore takes the "sublayer" branch:

```rust
#[cfg(target_os = "macos")]
let () = msg_send![view.as_ptr(), setWantsLayer: YES];
let root_layer: *mut Object = msg_send![view.as_ptr(), layer];
...
let is_metal_layer: BOOL = msg_send![root_layer, isKindOfClass: class!(CAMetalLayer)];
if is_metal_layer == YES {
    unsafe { StrongPtr::retain(root_layer) }
} else {
    // The view does not have a `CAMetalLayer` as the root layer (this
    // is the default for most views).
    ...
    unsafe { new_observer_layer(root_layer) }
}
```

- **Source:** [wgpu-hal 25.0.2 Metal `Surface::get_metal_layer`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/surface.rs) — lines 64-110 (vendored), continuing into `layer_observer.rs`. `new_observer_layer` is a rewrite of `raw-window-metal`: it creates a `CAMetalLayer` **subclass** (`WgpuObserverLayer`) and `addSublayer:`s it under the root layer, mirroring `contentsScale` and `bounds` via KVO — [wgpu-hal 25.0.2 `layer_observer.rs`](https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/layer_observer.rs), `new_observer_layer`, `fn class()`.

[INFERENCE] This matters to the pacing question because the presented content is a *child layer* composited by Core Animation into the window, not the window's backing layer directly. Apple's only statement about that machinery's timing is the `presentsWithTransaction` discussion: the default (`false`, which wgpu hard-codes) means "CAMetalLayer displays the output of a rendering pass to the display **as quickly as possible and asynchronously to any Core Animation transactions**. Core Animation doesn't guarantee that the Metal content arrives in the same frame as other Core Animation content" ([Apple — `CAMetalLayer.presentsWithTransaction`](https://developer.apple.com/documentation/quartzcore/cametallayer/presentswithtransaction)). There is no Apple document that quantifies a per-frame compositor penalty for a windowed sublayer.

### 14. The nominal panel rate is a *nominal* quantity. Two read-only sources exist; neither is a flip timestamp. [PRIMARY]

- `NSScreen.maximumFramesPerSecond`: "The maximum number of frames per second that the screen supports.", declared `var maximumFramesPerSecond: Int { get }` — **read-only**, macOS 12.0+ ([Apple — `NSScreen.maximumFramesPerSecond`](https://developer.apple.com/documentation/appkit/nsscreen/maximumframesperssecond)).
- `NSScreen.minimumRefreshInterval` / `maximumRefreshInterval`: "The shortest refresh interval that the screen supports." / "The largest refresh interval that the screen supports." — read-only, macOS 12.0+ ([Apple — `NSScreen.maximumRefreshInterval`](https://developer.apple.com/documentation/appkit/nsscreen/maximumrefreshinterval)).
- winit's equivalent, `MonitorHandle::refresh_rate_millihertz()`, is computed from `CGDisplayModeGetRefreshRate(current_mode) * 1000.0` and is *also* nominal: winit's own comment reads "`CGDisplayModeGetRefreshRate` returns 0.0 for any display that isn't a CRT" ([winit 0.30.13 `monitor.rs`](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/monitor.rs), `impl MonitorHandle`, `refresh_rate_millihertz`, lines 260-267 and 313-323). `CGDisplayMode` itself is "a set of properties (such as width, height, pixel depth, and refresh rate)" ([Apple — `CGDisplayMode`](https://developer.apple.com/documentation/coregraphics/cgdisplaymode)).
- For an actual flip timestamp the available calls are `CVDisplayLinkGetCurrentTime` — "You use this call to obtain the timestamp of the frame that is currently being displayed." ([Apple — `CVDisplayLinkGetCurrentTime(_:_:)`](https://developer.apple.com/documentation/corevideo/cvdisplaylinkgetcurrenttime(_:_:))) — and `CADisplayLink`'s `timestamp` / `targetTimestamp` ("The time interval that represents when the next frame displays.", macOS 14.0+) ([Apple — `CADisplayLink.targetTimestamp`](https://developer.apple.com/documentation/quartzcore/cadisplaylink/targettimestamp)).

[INFERENCE] With only nominal-rate sources, the residual ~12 µs between the measured p50 (16.679 ms) and `1/60 s` (16.6667 ms) cannot be attributed to either loop jitter or a panel whose true period differs slightly from its nominal one. At the default speed the median sits 12 µs *above* the nominal period while the mean (16.630 ms) sits 37 µs *below* it — a left-skewed distribution (short intervals beside the median). A saturated Fifo loop quantises intervals to multiples of the display period; a distribution that straddles the nominal period with a short-interval tail is the signature of a loop that alternates between "a drawable was already free, so the frame went out early" and "no drawable was free, so the acquire blocked past the grid" — i.e. the CPU is periodically running ahead of the display by up to `maximumDrawableCount - 1` frames and then paying for it. That is consistent with the x0-vs-x100 delta (sim 0.001 ms → p50 16.658; sim 0.123 ms → p50 16.679) being driven by the *work before the acquire*, and with `draw+present` measuring 16.4-16.5 ms of blocked time. It is offered as an inference, not a measurement: distinguishing it from a true panel period of ~16.6 ms requires a display-link or `presentedTime` timestamp, which this build cannot obtain (finding 9).

### 15. A 60 Hz panel cannot be asked for a different cadence; achievable rates are divisors of the panel rate. [PRIMARY]

This is the brief's last question and it has a clean first-party answer — from MetalKit's own rate control:

> "When your application sets its preferred frame rate, the view chooses a frame rate as close to that as possible based on the capabilities of the screen the view is displayed on. **To provide a consistent frame rate, the actual frame rate chosen is usually a factor of the maximum refresh rate of the screen.** For example, if the maximum refresh rate of the screen is `60` frames per second, that's also the highest frame rate the view sets as the actual frame rate. However, if you ask for a lower frame rate, the view might choose `30`, `20`, or `15` frames per second, or another factor, as the actual frame rate. Your application should choose a frame rate that it can consistently maintain. The default value is `60` frames per second."

- **Source:** [Apple — `MTKView.preferredFramesPerSecond`](https://developer.apple.com/documentation/metalkit/mtkview/preferredframespersecond).

So on a 60 Hz panel the sanctioned cadences are 60, 30, 20, 15, … — **there is no 90 Hz, no 61 Hz, and no sub-period rate**. The rate-limiting knobs that exist, and their owners:

| Knob | Effect | Owner |
|---|---|---|
| `MTKView.preferredFramesPerSecond` | MetalKit view redraw rate; quantised to a factor of the panel rate | [`MTKView.preferredFramesPerSecond`](https://developer.apple.com/documentation/metalkit/mtkview/preferredframespersecond) |
| `CADisplayLink.preferredFrameRateRange` | "A range of frequencies your app allows for frame updates, affecting how often the system invokes your delegate's callback." (macOS 14.0+) | [`CADisplayLink.preferredFrameRateRange`](https://developer.apple.com/documentation/quartzcore/cadisplaylink/preferredframeraterange) |
| `NSView.displayLink(target:selector:)` / `NSWindow.displayLink(target:selector:)` | "Returns a new display link whose callback will be invoked in-sync with the display the view[/window] is on." (macOS 14.0+) | [`NSView.displayLink(target:selector:)`](https://developer.apple.com/documentation/appkit/nsview/displaylink(target:selector:)) |
| `CGConfigureDisplayWithDisplayMode(_:_:_:_:)` | "Configures the display mode of a display." — the only way to change what the panel actually does (e.g. pick a 30 Hz mode) | [`CGConfigureDisplayWithDisplayMode`](https://developer.apple.com/documentation/coregraphics/cgconfiguredisplaywithdisplaymode(_:_:_:_:)) |
| `NSScreen.maximumFramesPerSecond` | **Query only** (`{ get }`) | [`NSScreen.maximumFramesPerSecond`](https://developer.apple.com/documentation/appkit/nsscreen/maximumframesperssecond) |

`CAMetalLayer` has **no** refresh-rate or frame-rate property at all: the CAMetalLayer surface is `maximumDrawableCount`, `allowsNextDrawableTimeout`, `displaySyncEnabled`, `presentsWithTransaction`, `device`, `pixelFormat`, `drawableSize`, `framebufferOnly`, `wantsExtendedDynamicRangeContent`, `colorspace`, `nextDrawable()`, `EDRMetadata` and the `CAMetalDrawable` pool ([Apple — `CAMetalLayer`](https://developer.apple.com/documentation/quartzcore/cametallayer) property list). On a windowed `CAMetalLayer`, "cadence" is not requestable — it is the panel's, and the only control the app has is to *present less often*.

### 16. Dawn (Chromium's WebGPU) does not have this knob at all; the wgpu/Dawn asymmetry runs the other way from the brief's assumption. [PRIMARY]

Verified negatives, all fetched 2026-09-12:

- `gpuweb/spec/index.bs` — **0 occurrences of "latency"**; there is no `presentMode` and no frame-latency member in `GPUCanvasConfiguration` ([gpuweb spec source](https://github.com/gpuweb/gpuweb/blob/main/spec/index.bs)).
- `webgpu-headers/webgpu.h` (6,771 lines, HTTP 200) — **no `desiredMaximumFrameLatency` / `maximumFrameLatency`** ([webgpu-headers `webgpu.h`](https://github.com/webgpu-native/webgpu-headers/blob/main/webgpu.h)).
- `google/dawn` `src/dawn/dawn.json` — no match; `src/dawn/native/Surface.{h,cpp}`, `src/dawn/native/SwapChain.cpp`, `src/dawn/native/vulkan/SwapChainVk.cpp` — no match; **all 47 files under `src/dawn/native/metal/`** — the only hits in the whole Metal backend are `SwapChainMTL.mm:86` `[*mLayer setDisplaySyncEnabled:(GetPresentMode() != wgpu::PresentMode::Immediate)]` and `SwapChainMTL.mm:113` `mCurrentDrawable = [*mLayer nextDrawable]` ([Dawn `SwapChainMTL.mm`](https://github.com/google/dawn/blob/main/src/dawn/native/metal/SwapChainMTL.mm)).

Concretely: **Dawn never sets `maximumDrawableCount` and never sets `allowsNextDrawableTimeout`**, so the layers it manages keep Apple's defaults (3 drawables; `nil` after one second). wgpu differs on both counts (findings 4 and 5). The name `desiredMaximumFrameLatency` exists only in the wgpu ecosystem — Rust `SurfaceConfiguration` and the C chained struct `WGPUSurfaceConfigurationExtras` ([wgpu-native `ffi/wgpu.h`](https://github.com/gfx-rs/wgpu-native/blob/trunk/ffi/wgpu.h); see also [gfx-rs/wgpu-native#401](https://github.com/gfx-rs/wgpu-native/issues/401)). **Whatever Chromium/Dawn caveat the brief refers to, it cannot bind this app: the field is not in Dawn, the WebGPU spec, or the cross-implementation header — and wgpu's Metal backend is one of the places where it *is* honoured.**

---

## Options for NeuroArena

Ranked by expected effect on the p50 interval per unit of cost. "Can it reach p50 <= 16.667 ms" is answered against the *measured* distribution, not against theory.

### 1. Establish the true panel period before spending anything on 12 µs. [can reach: N/A — it decides whether the target is real]

- **Mechanism:** the target `16.667 ms` is derived from nominal 60.00 Hz (`CGDisplayCopyDisplayMode`, `NSScreen.maximumFramesPerSecond`). Every available nominal source is read-only and none is a flip timestamp (finding 14). A `CADisplayLink` on the window's view reports `timestamp` (previous frame) and `targetTimestamp` ("when the next frame displays"), and reporting the *interval between consecutive `targetTimestamp`s* over a few hundred frames gives the panel's actual period, independent of the render loop.
- **Exact API:** `NSWindow.displayLink(target:selector:)` / `NSView.displayLink(target:selector:)` → `CADisplayLink`, `add(to:forMode:)`, `timestamp`, `targetTimestamp`, `preferredFrameRateRange` — macOS 14.0+ ([Apple](https://developer.apple.com/documentation/quartzcore/cadisplaylink/targettimestamp), [Apple](https://developer.apple.com/documentation/appkit/nswindow/displaylink(target:selector:))). Older equivalent: `CVDisplayLink` + `CVDisplayLinkGetCurrentTime` ([Apple](https://developer.apple.com/documentation/corevideo/cvdisplaylinkgetcurrenttime(_:_:))), available since macOS 10.4.
- **Preconditions:** a display link must be created on a window that is on a display; the callback must be read-only with respect to rendering (it is a measurement, not a pacer).
- **Expected effect on p50:** none by itself. It converts the present measurement from "compared against a nominal 16.6667 ms" into "compared against the display's actual period", which is the only way to know whether 16.679 ms is 12 µs of defect or 12 µs of measurement error.
- **Cost:** a second run-loop callback; no tearing, no latency, no power cost if it is temporary and diagnostic. Portability: macOS-only, which this app already is.
- **Verdict:** do this first. It is cheap, and findings 6/9 say the current measurement cannot distinguish the two hypotheses.

### 2. Cut the CPU work that runs between the wakeup and the acquire. [can reach: **yes, evidence-backed**]

- **Mechanism:** the interval is `(CPU work before the acquire) + (block until a drawable is free)`. Findings 2 and 6 show the block returns "usually at the display's next refresh interval"; leftover CPU work after the previous tick lands in the next period and can push the frame past the next grid point. The measured x0 run (sim 0.001 ms) already shows p50 **16.658 ms**, i.e. below the target, and the x100 run (sim 0.123 ms) shows **16.679 ms** — a ~21 µs p50 movement bought by ~120 µs of added per-frame work. There is also a fixed per-frame cost in the present path itself: wgpu commits an extra `MTLCommandBuffer` labelled `"(wgpu internal) Present"` every frame (finding 1).
- **Exact API:** nothing new — profile `App::tick` between the `RedrawRequested` dispatch and `surface.get_current_texture()` (`app/src/appstate.rs:632`). Candidates visible in the source: `build_frame()` before the acquire, and the `advance(dt)` sim step, which is already capped by `SIM_BUDGET`.
- **Preconditions:** the work must be measured with the same 600-frame probe; the x0/x100 pair already exists as the A/B.
- **Expected effect on p50:** the observed spread says the achievable range by this route alone is 16.658-16.679 ms, i.e. it crosses the target at the low end. Note the app already obeys Apple's own advice — "before retrieving a new drawable, you might perform other work on the CPU … Then, obtain the drawable and encode a command buffer" ([Apple — `CAMetalLayer`](https://developer.apple.com/documentation/quartzcore/cametallayer), "Keeping References to Drawables") — so the remaining win is *reducing* that work, not reordering it.
- **Cost:** none (no tearing, no added latency, less power). Portability unchanged.
- **Verdict:** the cheapest option with measured evidence for it, and the only one in this list that has already been observed to meet the target at the same loop structure.

### 3. `desired_maximum_frame_latency: 2 -> 1` (3 drawables -> 2). [can reach: **maybe; the sign is uncertain**

- **Mechanism:** one line in `app/src/appstate.rs:754`; wgpu-hal turns it into `CAMetalLayer.maximumDrawableCount = 2` (finding 4; legal value per finding 8). With two drawables the CPU can over-produce by at most one frame, so the "drawable was already free, present went out early" branch (finding 14) becomes rarer and the acquire blocks more often — the distribution tightens around the panel grid, at the cost of more blocked time.
- **Exact API:** `wgpu::SurfaceConfiguration::desired_maximum_frame_latency = 1`; clamped by wgpu-core into Metal's `1..=2` (`wgpu-core/src/device/global.rs:1845`).
- **Preconditions:** none beyond a re-`configure`. Per wgpu's own doc this setting "Remove[s] the ability of the CPU and GPU to run in parallel" when the backend does not implement wait-on-present — which is the case here (finding 5), so expect more CPU-GPU serialization, and note that with `allowsNextDrawableTimeout:false` a stall is unbounded.
- **Expected effect on p50:** unpredicted in sign by any source I found. It cannot raise the cadence (finding 4: the knob is depth, not period); it can only change how the interval distribution is shaped. Because the queue is `latency + 1`, latency 1 makes the acquire the only pacing point at all, which is more likely to *pin* the median to the grid — but also to raise p99/max.
- **Cost:** input-to-photon latency *improves* (one fewer frame of queueing); blocked time and p99 risk increase; no tearing; no portability change. Cheap to test with the existing probe.
- **Verdict:** worth one measured A/B, and it is the only knob in this list that changes the drawable pool rather than the loop.

### 4. `PresentMode::Immediate` (i.e. `setDisplaySyncEnabled(false)`). [can reach: **no** — disproved by measurement; the acquire stays cadence-bound]

- **Mechanism (unchanged):** finding 3 — `Immediate` maps to `displaySyncEnabled = false`, and Apple documents only this of the property: "If `false`, the layer presents new content more quickly, but possibly with brief visual artifacts (tearing)." That is a statement about how fast *content is presented*, not about how often a drawable becomes available — and in a window the drawable is what the loop waits on (finding 2).
- **Exact API:** `wgpu::PresentMode::Immediate` in `SurfaceConfiguration::present_mode`; advertised on Metal whenever `can_set_display_sync` (macOS >= 10.13) — `metal/adapter.rs:369-373`.
- **Preconditions:** none. The surface configures successfully with `Immediate`, so wgpu's `Fifo => true / Immediate => false` path ran and `setDisplaySyncEnabled:false` did reach the layer (finding 3).
- **Measured effect on p50** (probe in `App::draw`, 120 warm-up frames discarded, 900 measured, 3 runs per mode, same machine, same repo, release build; probe since reverted — same binary and machine as the table in "The question"):

| config | p50 interval | p50 fps | mean interval | mean fps |
|---|---|---|---|---|
| `PresentMode::Fifo`, speed x100 | 16.706 / 16.710 / 16.721 ms | 59.8-59.9 | 16.666 / 16.667 / 16.666 ms | 60.0 |
| `PresentMode::Immediate`, speed x100 | 16.705 / 16.707 / 16.726 ms | 59.8-59.9 | 16.481 / 16.501 / 16.556 ms | 60.4-60.7 |
| offscreen render, no acquire/present, x100 | 1.028 ms | 973 | 1.042 ms | 960 |

  **The p50 did not move: the two modes differ by less than 20 µs**, inside the run-to-run spread, and both sit ~40 µs above one nominal panel period. Only the mean improved (~180 µs), i.e. a few frames were delivered earlier — the acquire still blocked for ~14.4 ms at p50. The third row is the control that identifies what is being waited on: with no acquire and no present, the same loop runs at p50 1.03 ms, so essentially all of the ~16.7 ms interval is the acquire, and turning off display sync does not shorten it.
- **Why the layer setting does not unpin a window** [INFERENCE from primaries; **no Apple document states this case** — the measurement is the evidence]: Apple's drawable guidance is written with no reference to `displaySyncEnabled`. "Drawables … exist within a limited and reusable resource pool and may or may not be available when requested by your app. If there is no drawable available at the time of your request, the calling thread is blocked until a new drawable becomes available (which is usually at the next display refresh interval)", and for a `CAMetalLayer`-backed view "the `presentDrawable:` method schedules the actual presentation to occur at the next display refresh interval" — [Apple — Metal Best Practices Guide: Drawables](https://developer.apple.com/library/archive/documentation/3DDrawing/Conceptual/MTLBestPracticesGuide/Drawables.html) (archived). The layer reuses a drawable only once it "isn't onscreen" ([`CAMetalLayer`](https://developer.apple.com/documentation/quartzcore/cametallayer), "Keeping References to Drawables"). Together: the pool recycles on the compositor's cadence, and the compositor is not the thing `displaySyncEnabled` configures. So sync-off lets a frame be *presented* sooner within its refresh (which is where the small mean improvement comes from) without making drawables *available* sooner, and the acquire keeps blocking for about a period. Apple's `displaySyncEnabled` page covers presentation speed, not pool recycling, so the statements above are the primaries the result is consistent with rather than a source that predicts it.
- **Expected effect on p50:** none measured; it cannot reach `p50 <= 16.667 ms` by this route. Windowed compositor pacing governs the median.
- **Cost:** tearing is still paid, and on the median it buys nothing; a small mean improvement; higher power; the vsync-aligned phase is given up while the cadence is unchanged, so motion does not benefit.
- **Verdict:** do not use. It stays in the list because the knob exists and because this measurement is the cleanest demonstration available that, in a window, the panel grid is enforced by the drawable pool and the compositor rather than by the layer's present-sync setting.

### 5. Stop treating the interval as the goal; pace presentation from a display link and make motion insensitive to the interval. [can reach: yes — by removing the need]

- **Mechanism:** instead of driving `RedrawRequested` from `about_to_wait` (finding 11, one run-loop iteration per frame with no documented latency bound) and letting `nextDrawable()` be the de-facto pacer, take the pacer explicitly: a `CADisplayLink` on the window (`NSWindow.displayLink(target:selector:)`, macOS 14+) or `CVDisplayLink`, and render only when the link fires; present from that callback. Interpolate the simulation between fixed `sim::DT` steps so that a non-uniform interval cannot leak into motion.
- **Exact API:** `CADisplayLink` via `NSWindow.displayLink(target:selector:)`, `add(to:forMode:)`, `timestamp` / `targetTimestamp`; or `CVDisplayLink` + `CVDisplayLinkCreateWithCGDisplay` / `CVDisplayLinkGetCurrentTime` on older systems. The app's sim already steps at fixed `sim::DT = 1/60` with `advance(dt)`, so the second half of this option is mostly already in place.
- **Preconditions:** a display link needs the window on a display; driving the render from a link callback means either rendering off the winit main thread or re-entering the event loop, so this is the invasive option.
- **Expected effect on p50:** the *interval* stops being the quantity that matters; a link-paced loop presents at most once per `targetTimestamp`, so the interval becomes the panel period as reported by the link rather than by the drawable pool. This is also the only route that yields a *timestamp* for each tick (`targetTimestamp`) without wgpu present feedback (finding 9).
- **Cost:** architectural (threading / event-loop re-entry), no tearing, no added input latency beyond one frame, power roughly unchanged. Portability: macOS-only — but so is this app.
- **Corroboration (practice, not authority):** on the wgpu tracking issue, a developer building a voxel engine with "the usual fixed tick + render interpolation loop" reports the interpolation alpha derived from CPU-side frame timestamps producing "1-3ms of jitter [that] aliases into visible translation micro-stutter at a rock steady 60fps (worst when refresh rate == tick rate)" and states they work around it "with a software PLL that re-derives the vsync grid statistically from measured intervals" — [ANECDOTE] [gfx-rs/wgpu#2869 comment, 2026-07-04](https://github.com/gfx-rs/wgpu/issues/2869#issuecomment-4881751622). Also on that thread, the wgpu maintainer who owns the WSI work: "I have personally given up on trying to figure out when the next present will happen as there are way too many factors, and instead focused on presenting a smooth series of frames and with corresponding present times." — [ANECDOTE] [comment, 2024-08-27](https://github.com/gfx-rs/wgpu/issues/2869#issuecomment-2313702193).

### 6. Obtain real present timestamps. [can reach: no, not at the pinned version]

- **Mechanism:** with `presentedTime` per frame, "the interval between the last and current presentation times" is directly measurable (Apple's own reason for the API, finding 9), which removes the need to infer the grid.
- **Exact API:** `MTLDrawable.addPresentedHandler(_:)` + `MTLDrawable.presentedTime` (macOS 10.15.4+) — both bound in `metal 0.31.0` (`drawable.rs:32-38`) but unreachable, because wgpu-hal keeps the `MTLDrawable` private and `SurfaceTexture::present` returns `()`.
- **Preconditions / availability:** absent in wgpu 25 (verified by grep across all four crates), absent on wgpu 29.0.4 (#9856), unresolved as of 2026-09 (issue open, four API questions unanswered). The escape hatches the maintainer points at — `Adapter::open_with_callback` and `Surface::set_next_present_chain` ([PR #9847](https://github.com/gfx-rs/wgpu/pull/9847)) — are wgpu **30**-era APIs and do not exist in 25.0.2.
- **Expected effect on p50:** none directly; it makes the measurement honest.
- **Cost:** a wgpu upgrade, or a vendor patch to wgpu-hal.
- **Verdict:** the correct long-term answer, unavailable now. The practical stand-in is option 5's `targetTimestamp`, with the caveat the #9856 author gives: it "only estimates the next display tick; it cannot prove that the particular drawable was shown".

---

## What does not work

1. **"`present()` blocks on the next vsync."** It does not block at all: `SurfaceTexture::present` dispatches to `Queue::present`, which commits a command buffer and returns `Ok(())` (finding 1). The blocking call is `Surface::get_current_texture()`, and wgpu's `Fifo` documentation says so explicitly — "Calls to `Surface::get_current_texture()` will block until there is a spot in the queue" (finding 2).
2. **"`get_current_texture()` reports `SurfaceError::Timeout` if the display stalls."** Not in practice on Metal. The variant is wired up (wgpu-core maps HAL `Ok(None)` to `Status::Timeout`, `present.rs:217`; wgpu maps that to `SurfaceError::Timeout`), but it can only be produced by `nextDrawable()` returning `nil`, and wgpu disables the property that makes it do so: `allowsNextDrawableTimeout:false` means the call "waits indefinitely for a drawable to become available" rather than returning `nil` after one second (finding 5). The 1000 ms the core layer passes is discarded by the hal (`_timeout_ms: Option<std::time::Duration>, //TODO`). So a stalled present path presents as an unbounded main-thread block, not as an error to match on.
3. **"`desired_maximum_frame_latency = 0` for minimum latency."** Clamped: wgpu-types says "A value of 0 is generally not supported and always clamped to a higher value", and wgpu-core clamps into Metal's `1..=2` (finding 4).
4. **"A deeper queue (`desired_maximum_frame_latency = 3`) smooths the interval."** Not on Metal: the range is `1..=2` (finding 4) because `CAMetalLayer.maximumDrawableCount` accepts "2 or 3 only; if you pass a different value, Core Animation ignores the value and throws an exception" (finding 8). `3` would mean `maximumDrawableCount = 4` — an exception, not a deeper queue.
5. **"Use `PresentMode::Mailbox` (or `FifoRelaxed`) to get a 1-frame queue with no tearing."** Unavailable: Metal advertises only `[Fifo, Immediate]`, and the configure `match` ends in `unreachable!("Unsupported present mode: {m:?}")` for anything else (finding 3). wgpu documents `Mailbox` as "DX12 on Windows 10, NVidia on Vulkan and Wayland on Vulkan" and `FifoRelaxed` as "AMD on Vulkan" ([`PresentMode`](https://docs.rs/wgpu-types/25.0.0/wgpu_types/enum.PresentMode.html)).
6. **"Set `CAMetalLayer` properties directly for the layer this app is drawing into."** Not reachable at the pinned version: `wgpu_hal::metal::Surface`'s layer field is private — `render_layer: Mutex<metal::MetalLayer>` — with no accessor, and `Surface::as_hal` hands back only that opaque value (`wgpu-hal-25.0.2/src/metal/mod.rs:377-385`, `wgpu-25.0.2/src/api/surface.rs:139-158`). Neither `displaySyncEnabled`, nor `allowsNextDrawableTimeout`, nor `presentsWithTransaction`, nor `maximumDrawableCount` can be touched outside `Surface::configure`.
7. **"`MTLPresentMode` lets you choose a present mode per command buffer."** `MTLPresentMode` is not public API: `/documentation/Metal/MTLPresentMode` 404s on Apple's documentation service, and neither the `metal` crate nor wgpu-hal mentions it (finding 7). The nearest real primitives are `MTLCommandBuffer.present(_:)` / `present(_:atTime:)` and `CAMetalLayer.displaySyncEnabled`.
8. **"`presentsWithTransaction = true` removes compositor asynchrony."** It is hard-coded `false` in wgpu 25 — `present_with_transaction: false` in `Surface::new` (`metal/surface.rs:27-36`), written to the layer as `set_presents_with_transaction(self.present_with_transaction)` (line 162) — and there is no public API to set it (finding 7/option 6). It is also not obviously desirable: Apple's description of the default is that the layer "displays the output of a rendering pass to the display as quickly as possible and asynchronously to any Core Animation transactions"; the transaction mode exists to keep Metal content in step with *other* Core Animation content, at the cost of waiting on the transaction.
9. **"Ask the panel for 90 Hz / a custom cadence."** Not available: achievable frame rates are factors of the panel's maximum rate ("the view might choose `30`, `20`, or `15` frames per second"; on a 60 Hz panel, 60 is the ceiling), `NSScreen.maximumFramesPerSecond` is a read-only query, and the only way to change what the panel does is a display-mode change via `CGConfigureDisplayWithDisplayMode` (finding 15).
10. **"Drive the loop with `setNeedsDisplay` for tighter coupling to AppKit's drawing loop."** winit's macOS `request_redraw` does not use it (finding 10), and AppKit's own `NSView` semantics are weaker, not stronger: "View objects marked as needing display are automatically redisplayed on each pass through the application's event loop" ([Apple — `NSView.setNeedsDisplay(_:)`](https://developer.apple.com/documentation/appkit/nsview/setneedsdisplay(_:))). Calling `setNeedsDisplay` yourself would add an AppKit redisplay hop *on top of* the winit hop, not replace it.
11. **"`ControlFlow::Poll` will tighten the redraw hop."** It only changes the waker timer: `Poll` arms an immediately-firing `CFRunLoopTimer`; `Wait` disarms it. `RedrawRequested` is dispatched from the `kCFRunLoopBeforeWaiting` observer either way, and `request_redraw` calls `CFRunLoopWakeUp` either way, so the structural cost of one run-loop iteration per frame is unchanged (finding 12).
12. **"The present path is where the 12 µs went, so change the present path."** The present path has exactly two knobs at this version (drawable count via `desired_maximum_frame_latency`, and vsync via `present_mode`), neither of which changes the number of frames the display consumes per second (finding 4). The measurable lever in the author's own data is the per-frame CPU work before the acquire (option 2).
13. **Secondary sources as authority.** This file cites no blog, Reddit or Stack Overflow claim as a mechanism. The two community statements that do appear (option 5) are labelled [ANECDOTE] and are used as reports about *practice*, with the mechanism they describe owned by the wgpu issue thread they come from.

---

## Sources

Every URL below was fetched or HTTP-probed on 2026-09-12. Crate sources were additionally read from the vendored copies at `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/<crate>-<version>/`, which are the exact versions in `Cargo.lock`.

**Apple — Metal / QuartzCore / AppKit**

- https://developer.apple.com/documentation/metal/onscreen-presentation — owns "`nextDrawable()` blocks the calling thread until a drawable becomes available — usually at the display's next refresh interval".
- https://developer.apple.com/documentation/quartzcore/cametallayer/nextdrawable() — owns "Waits until a Metal drawable is available, and then returns it" and the one-second/nil discussion.
- https://developer.apple.com/documentation/quartzcore/cametallayer/maximumdrawablecount — owns "You can set this value to 2 or 3 only … The default value is 3".
- https://developer.apple.com/documentation/quartzcore/cametallayer/allowsnextdrawabletimeout — owns "waits indefinitely for a drawable to become available".
- https://developer.apple.com/documentation/quartzcore/cametallayer/displaysyncenabled — owns the vsync/tearing definition and the `true` default.
- https://developer.apple.com/documentation/quartzcore/cametallayer/presentswithtransaction — owns "asynchronously to any Core Animation transactions" and the `false` default.
- https://developer.apple.com/documentation/quartzcore/cametallayer — owns "The layer reuses a drawable only if it isn't onscreen and there are no strong references to it" and Apple's "do other CPU work before retrieving a new drawable" advice.
- https://developer.apple.com/documentation/metal/mtldrawable/present() — owns "presents its contents as soon as possible after all scheduled render or write requests for that drawable are complete".
- https://developer.apple.com/documentation/metal/mtldrawable/present(at:) — owns `present(at:)` and "The Mach absolute time at which the drawable should be presented" (macOS 10.11+).
- https://developer.apple.com/documentation/metal/mtlcommandbuffer/present(_:) — owns "calls the drawable's `present()` method after the command queue schedules the command buffer … by adding a completion handler by calling its own `addScheduledHandler(_:)`".
- https://developer.apple.com/library/archive/documentation/3DDrawing/Conceptual/MTLBestPracticesGuide/Drawables.html — owns "the calling thread is blocked until a new drawable becomes available (which is usually at the next display refresh interval)" and "the `presentDrawable:` method schedules the actual presentation to occur at the next display refresh interval"; neither sentence is conditioned on `displaySyncEnabled`.
- https://developer.apple.com/documentation/metal/mtldrawable/presentedtime — owns "The host time, in seconds, when the drawable was displayed onscreen" and the `0`-means-dropped rule (macOS 10.15.4+).
- https://developer.apple.com/documentation/metal/mtldrawable/addpresentedhandler(_:) — owns "derive the interval between the last and current presentation times … determines the app's frame rate".
- https://developer.apple.com/documentation/metal/mtldrawable — protocol overview.
- https://developer.apple.com/documentation/quartzcore/cametaldrawable — "A Metal drawable associated with a Core Animation layer."
- https://developer.apple.com/documentation/quartzcore/cadisplaylink — "synchronize your drawing to the refresh rate of the display".
- https://developer.apple.com/documentation/quartzcore/cadisplaylink/targettimestamp — owns "when the next frame displays" (macOS 14.0+).
- https://developer.apple.com/documentation/quartzcore/cadisplaylink/preferredframeraterange — owns the callback-frequency range knob (macOS 14.0+).
- https://developer.apple.com/documentation/quartzcore/cadisplaylink/invalidate() — owns display-link teardown.
- https://developer.apple.com/documentation/appkit/nsview/displaylink(target:selector:) — owns "invoked in-sync with the display the view is on" (macOS 14.0+).
- https://developer.apple.com/documentation/appkit/nswindow/displaylink(target:selector:) — same for a window.
- https://developer.apple.com/documentation/metalkit/mtkview/preferredframespersecond — owns "the actual frame rate chosen is usually a factor of the maximum refresh rate … 30, 20, or 15".
- https://developer.apple.com/documentation/metalkit/mtkview — MetalKit view overview.
- https://developer.apple.com/documentation/appkit/nsscreen/maximumframesperssecond — owns the read-only `maximumFramesPerSecond { get }` (macOS 12.0+).
- https://developer.apple.com/documentation/appkit/nsscreen/maximumrefreshinterval — owns "The largest refresh interval that the screen supports" (macOS 12.0+).
- https://developer.apple.com/documentation/appkit/nsscreen/minimumrefreshinterval — owns "The shortest refresh interval that the screen supports".
- https://developer.apple.com/documentation/appkit/nsview/setneedsdisplay(_:) — owns "automatically redisplayed on each pass through the application's event loop".
- https://developer.apple.com/documentation/appkit/nsview/displayifneeded() — the AppKit redisplay call.
- https://developer.apple.com/documentation/corevideo/cvdisplaylink — CVDisplayLink reference (macOS 10.4+).
- https://developer.apple.com/documentation/corevideo/cvdisplaylinkgetcurrenttime(_:_:) — owns "the timestamp of the frame that is currently being displayed".
- https://developer.apple.com/documentation/coregraphics/cgdisplaymode — owns "a set of properties (such as width, height, pixel depth, and refresh rate)".
- https://developer.apple.com/documentation/coregraphics/cgdisplaycopydisplaymode(_:) — reads a display's current mode.
- https://developer.apple.com/documentation/coregraphics/cgconfiguredisplaywithdisplaymode(_:_:_:_:) — owns "Configures the display mode of a display" (the only cadence change).
- https://developer.apple.com/documentation/corefoundation/cfrunloopwakeup(_:) — owns "Wakes a waiting CFRunLoop object".
- https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/RunLoopManagement/RunLoopManagement.html — run loop reference linked from winit's `observer.rs` module doc.
- https://developer.apple.com/documentation/metal/mtlpresentmode — **not found (404)** on 2026-09-12: `MTLPresentMode` is not publicly documented.
- https://developer.apple.com/documentation/metal/creating-a-custom-metal-view — Apple's sample that drives a custom `CAMetalLayer` view from a display link, "synchronizing updates to the display's refresh interval".

**wgpu (25.0.2 / wgpu-core 25.0.2 / wgpu-hal 25.0.2 / wgpu-types 25.0.0) and wgpu-native**

- https://docs.rs/wgpu-types/25.0.0/wgpu_types/enum.PresentMode.html — owns the `Fifo` queue description and "Calls to `Surface::get_current_texture()` will block until there is a spot in the queue"; source `wgpu-types-25.0.0/src/lib.rs:5138-5210`.
- https://docs.rs/wgpu-types/25.0.0/wgpu_types/struct.SurfaceConfiguration.html — owns the `desired_maximum_frame_latency` documentation (clamping, the three backend strategies, the CPU-GPU serialization warning, the #2869 link); source `wgpu-types-25.0.0/src/lib.rs:5445-5455`.
- https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/surface.rs — `Surface::configure` lines 143-179 (display sync, transaction flag, `maximum_drawable_count`, `setAllowsNextDrawableTimeout:false`), `acquire_texture` lines 186-198 (ignored timeout, `next_drawable()`), `get_metal_layer` lines 64-110.
- https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/mod.rs — `Queue::present` lines 468-492 ("(wgpu internal) Present"), private `Surface::render_layer` line 378, private `SurfaceTexture::drawable` line 394.
- https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/adapter.rs — `present_modes: [Fifo, Immediate]` and `maximum_frame_latency: 1..=2` lines 358-373; `can_set_display_sync` / `can_set_next_drawable_timeout` lines 847-848.
- https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-hal/src/metal/layer_observer.rs — owns the sublayer `WgpuObserverLayer` (a `CAMetalLayer` subclass added with `addSublayer:`).
- https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-core/src/present.rs — `const FRAME_TIMEOUT_MS: u32 = 1000;` line 29 and the `acquire_texture(Some(...))` call.
- https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu-core/src/device/global.rs — `desired_maximum_frame_latency.clamp(...)` lines 1845-1848.
- https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu/src/api/surface_texture.rs — `SurfaceTexture` fields, `pub fn present(mut self)` (line 37), `Drop`/discard.
- https://github.com/gfx-rs/wgpu/blob/v25.0.2/wgpu/src/api/surface.rs — `get_current_texture` and `SurfaceStatus` mapping (lines 106-116), `get_default_config` (`desired_maximum_frame_latency: 2`, line 70), `as_hal`.
- https://docs.rs/wgpu/25.0.2/wgpu/struct.SurfaceTexture.html — public surface of `SurfaceTexture` at 25.0.2 (no present timestamp).
- https://docs.rs/wgpu/25.0.2/wgpu/type.SurfaceConfiguration.html — the re-exported configuration type.
- https://docs.rs/wgpu/25.0.2/wgpu/enum.SurfaceError.html — `Timeout` variant text.
- https://github.com/gfx-rs/wgpu/issues/2869 — tracking issue "Extended Presentation API Investigation": the WSI capability table for `CAMetalLayer`, the `maximum_latency` proposal ("swapchain frame count to `value + 1` … or uses a wait-for-present in the acquire method"), monitor-statistics proposal, and the maintainer/community comments quoted above.
- https://github.com/gfx-rs/wgpu/issues/9856 — owning issue for the absence of per-frame present feedback ("On wgpu 29.0.4, `SurfaceTexture::present(self)` returns `()` …"), the Metal-first `addPresentedHandler` implementation sketch, and the `present()`-vs-`targetTimestamp` p50 7.613 ms anecdote.
- https://github.com/gfx-rs/wgpu/pull/9847 — `set_next_present_chain` (wgpu 30-era escape hatch referenced by the maintainer).
- https://github.com/gfx-rs/wgpu-native/blob/trunk/ffi/wgpu.h — `WGPUSurfaceConfigurationExtras.desiredMaximumFrameLatency` and its "number of monitor refreshes between `wgpuSurfaceGetCurrentTexture` and presentation" definition.
- https://github.com/gfx-rs/wgpu-native/issues/401 — confirms the field is a wgpu-native extension struct member.

**winit 0.30.13**

- https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/window_delegate.rs — `request_redraw` (line 916) calls `queue_redraw`.
- https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/app_state.rs — `queue_redraw` (297-303), `handle_redraw` (278-293), `cleared` (373-420) with the redraw drain → `AboutToWait` → waker ordering, `control_flow: Cell::new(ControlFlow::default())` (96).
- https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/observer.rs — `RunLoop::wakeup` = `CFRunLoopWakeUp` (111-113), observer registration at `kCFRunLoopAfterWaiting` / `kCFRunLoopBeforeWaiting` (206-231), `EventLoopWaker` timer and `start_at` (236-306).
- https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/view.rs — `-[NSView drawRect:]` (202-208); no `setNeedsDisplay`, no `CAMetalLayer`.
- https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/macos/monitor.rs — `refresh_rate_millihertz` via `CGDisplayModeGetRefreshRate` (260-267, 313-323).
- https://github.com/rust-windowing/winit/blob/v0.30.13/src/window.rs — `Window::request_redraw` doc: "no strong guarantees about when exactly a `RedrawRequest` event will be emitted", macOS absent from the platform list (572-596).
- https://docs.rs/winit/0.30.13/winit/event_loop/enum.ControlFlow.html — "Defaults to `Wait`".
- https://docs.rs/winit/0.30.13/winit/application/trait.ApplicationHandler.html — `about_to_wait` / `RedrawRequested` handler contract.

**metal 0.31.0**

- https://docs.rs/metal/0.31.0/metal/struct.MetalLayer.html — `set_maximum_drawable_count`, `display_sync_enabled`, `presents_with_transaction`, `next_drawable`, `contents_scale`.
- https://docs.rs/metal/0.31.0/metal/struct.Drawable.html — `present`, `drawable_id`, `add_presented_handler`, `presented_time`.
- https://docs.rs/metal/0.31.0/metal/struct.CommandBuffer.html — `present_drawable` (the only presentation entry point bound).

**WebGPU / Dawn (comparison)**

- https://github.com/gpuweb/gpuweb/blob/main/spec/index.bs — WebGPU spec source; 0 occurrences of "latency", no `presentMode` in `GPUCanvasConfiguration`.
- https://github.com/webgpu-native/webgpu-headers/blob/main/webgpu.h — canonical C header; no frame-latency member.
- https://github.com/google/dawn/blob/main/src/dawn/native/metal/SwapChainMTL.mm — the only `CAMetalLayer` knob Dawn's Metal backend sets is `setDisplaySyncEnabled` (line 86); `nextDrawable` at line 113.
- https://raw.githubusercontent.com/google/dawn/main/src/dawn/dawn.json — Dawn's IDL: no frame-latency member (grep, 0 matches).

---

## Report back

- **`present()` is not the pacing point and does not block.** In wgpu 25 `SurfaceTexture::present` only flags and dispatches; the Metal `Queue::present` commits one extra command buffer and returns. The frame interval is spent in `Surface::get_current_texture()` → `CAMetalLayer.nextDrawable()`, which wgpu calls with `allowsNextDrawableTimeout = false`, so it waits **indefinitely** rather than returning after the one-second default. Apple owns the semantics: "`nextDrawable()` blocks the calling thread until a drawable becomes available — usually at the display's next refresh interval" ([Onscreen presentation](https://developer.apple.com/documentation/metal/onscreen-presentation)). Note the error semantics that follow: `SurfaceError::Timeout` is wired up but unreachable in practice on Metal, so a stalled present path is an unbounded main-thread block, not a matchable error.
- **`desired_maximum_frame_latency` is a pipeline-depth knob, not a cadence knob.** On Metal it is literally `CAMetalLayer.maximumDrawableCount = latency + 1` (`wgpu-hal/src/metal/surface.rs:171`), clamped by wgpu-core into Metal's `1..=2` because Apple's property accepts "2 or 3 only". Its C-side documentation defines it as "the number of monitor refreshes between `wgpuSurfaceGetCurrentTexture` and presentation" — so 2 → 1 changes phase/latency and the acquire-block distribution, not the number of frames the display consumes per second. It is worth exactly one measured A/B, with the caveat that lower depth plus an infinite `nextDrawable` wait is the risky combination.
- **The only measured lever in the author's own data is per-frame CPU work before the acquire.** x0 (sim 0.001 ms) already reaches p50 16.658 ms — below the target — on the identical loop; x100 (sim 0.123 ms) sits at 16.679 ms. The distribution at the default speed is left-skewed (mean 16.630 < p50 16.679), which matches a loop that periodically runs ahead of the display by up to `drawableCount - 1` frames and then blocks. Trimming the work between the wakeup and `get_current_texture()` is the cheapest route that has already been observed to hit the target.
- **wgpu 25 cannot tell you when a frame was displayed, and the absence has named owners.** Zero hits for `presented_time` across wgpu/wgpu-core/wgpu-hal/wgpu-types 25; `present()` returns `()`. Owning issues: [#2869](https://github.com/gfx-rs/wgpu/issues/2869) (tracking; its `CAMetalLayer` row already marks Present Time as available "through callbacks" and Monitor Frequency as available "via NSScreen") and [#9856](https://github.com/gfx-rs/wgpu/issues/9856) (Metal-first feedback API, still unresolved; the escape hatches the maintainer names are wgpu 30-era). Apple's `presentedTime` + `addPresentedHandler` are bound in `metal` 0.31.0 but unreachable through wgpu.
- **The winit redraw hop is one full CFRunLoop iteration and macOS has no documented latency bound.** `request_redraw` does not call `setNeedsDisplay`; it pushes onto `pending_redraw` and calls `CFRunLoopWakeUp`. `RedrawRequested` is drained in the `kCFRunLoopBeforeWaiting` observer immediately *before* `AboutToWait`, so `about_to_wait`'s request always lands in the next iteration. `ControlFlow::Poll` vs `Wait` only changes whether winit's 0.1 µs `CFRunLoopTimer` is armed — this app runs the default `Wait` branch, i.e. the timer is disarmed and `CFRunLoopWakeUp` is what keeps the loop turning. winit's own `request_redraw` docs say there are "no strong guarantees" about when the event arrives and list Windows/iOS/Wayland/Web — not macOS.
- **A 60 Hz panel cannot be asked for a different cadence; rates must be factors of 60.** Apple's `MTKView.preferredFramesPerSecond` discussion owns the rule ("the actual frame rate chosen is usually a factor of the maximum refresh rate … `30`, `20`, or `15`"); `NSScreen.maximumFramesPerSecond` and `maximumRefreshInterval`/`minimumRefreshInterval` are read-only queries (macOS 12+), and `CAMetalLayer` has no rate property at all. The only paths to a display-time timestamp without wgpu present feedback are `CADisplayLink` (`NSWindow.displayLink(target:selector:)`, macOS 14+, with `timestamp`/`targetTimestamp`) and `CVDisplayLinkGetCurrentTime`. **Open item for the main agent:** whether the residual ~12 µs is loop jitter or a true panel period slightly off nominal cannot be settled from this build — a `CADisplayLink.targetTimestamp` interval measurement should precede any present-path change.
