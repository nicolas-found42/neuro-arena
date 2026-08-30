# ADR 0001 — Locked Arcade Sketch — Win98 Bevel + Rough ESM Mandatory, No Vanilla Fallback

- **Status:** Accepted — locked production (main), no fallback.
- **Date:** 2026-08-27

## Context
Synthesis of 48 candidates (SYNTHESIS-GRAPHICS.md) within GH Pages static constraints — no build, vanilla ES modules, single-file CDN only (`access-control-allow-origin: *`, MIT/OFL, keyless) — identified Tier1 composition 98.css + NES.css (~9KB) + Rough.js bundled ~9KB ESM as optimal weight-to-feel for seed 1409546675 pixel parity.

## Decision
Lock variant C **arcade-sketch** full chrome verbatim into `main`: `normalize 8.0.1 (cdnjs)` → `98.css 0.1.21 (unpkg)` → `NES.css 2.3.0 (unpkg)` → single Google Fonts link `Space Mono` + `Press Start 2P` (OFL), canvas `FONT` = `'Space Mono', 'Press Start 2P', monospace`, importmap `{"imports":{"roughjs":"https://cdn.jsdelivr.net/npm/roughjs@4.6.6/bundled/rough.esm.js"}}` consumed as `import rough from 'roughjs'` in `js/render.js` with singleton `rough.generator({roughness:1.2,bowing:1})` and cached `asteroid._roughDrawable` via `rough.generator().polygon(verts,{stroke:'#1a1a1a',fill:'#9aa4b0',fillStyle:'hachure',roughness:1.1,bowing:1,strokeWidth:1,seed})` drawn per frame via `rough.canvas(ctx.canvas).draw()` per `seamCopies(x,y,r)→4× ±W/±H` copy through `ctx.save/translate/restore` after `setupHiDPI` transform (0 per-frame Drawable alloc, GC-safe at ×10000, invalidated on shatter), plus arcade Target1 arena inset highlight `rgba(255,255,255,0.06) rect 0.5,0.5,959,599` + `rgba(0,0,0,0.22) rect 1.5,1.5,957,597` and ship 1px white inset circle arc (plain arc, as the prototype drew); delete vanilla jitter polygon entirely (10 verts jitter 0.75–1.25 stroke `#9aa4b0` 1.5) — no gated `if (useRough)` fallback.

## Consequences
Why: 0KB vanilla baseline rejected — sketch bevel greatly improves feel at modest 9KB gz; seeded Rough cache preserves determinism while hard-to-reverse CDN lock prevents drift; vs gated fallback rejected for retainable dead code and vs per-frame alloc rejected for GC pressure. Alternatives considered (gated fallback, per-frame drawable alloc, CRT/nebula variants) archived on `prototype/graphics-variants`. Consequence: `prototype-graphics.html` removed from `main`, `verify.mjs` updated atomically to assert Rough path/cache/seam draws and no vanilla stroke, `setupHiDPI` DPR cap / `seamCopies` 4-wrap / throttling >16× / logical `960×600` invariants preserved.
## Verification
`node verify.mjs` 80-green exercises the headless `node_modules/roughjs` stub's echo (ADR-locked option set incl. `hachure` fill, `__roughDrawCalls` seam 4×/1×, `_roughDrawable` cache), behavioral guards for the Target1 inset bevel rects and >16× throttling, and locked chrome/invariant constants (canvas `FONT`, 120-dot starfield, export surface, `seamCopies` wraps, logical `960×600`) — wiring-regression guards, not real jsDelivr Rough ESM compatibility. Real API is proven by browser smoke at `http://127.0.0.1:8899/index.html?seed=1409546675` (importmap `rough.esm.js 200 CORS *`, `normalize`/`98.css`/`NES` all 200 `*`, canvas `pixelAvg ~52`, no `pageerror`, favicon 404 only). Stub creation in `verify.mjs` now `catch (e){ warn + throw }` so fresh-clone failures are loud, not a confusing `Cannot find package 'roughjs'`.


## References
- Synthesis: `docs/research/SYNTHESIS-GRAPHICS.md`
- Prototype: `prototype/graphics-variants` (throwaway) `prototype-graphics.html` — deleted from `main`
- Prototype weight budgets verbatim: 0KB vanilla; Kontra 12.4KB (~5KB gz); regl 29KB (~9KB gz); Rough ~9KB; Two.js 44KB (~14–15KB gz); Pixi 350KB (~45–60KB gz)
- CDN: `https://cdn.jsdelivr.net/npm/roughjs@4.6.6/bundled/rough.esm.js` (export default, single-file, CORS *)
- Fonts: `https://fonts.googleapis.com/css2?family=Space+Mono:wght@400;700&family=Press+Start+2P&display=swap` (OFL)
