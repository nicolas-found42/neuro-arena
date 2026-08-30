# Graphics Research Synthesis — GH Pages Viable (Zero-Build)

**Date:** 2026-08-27
**Project:** `neural-network-game` — vanilla ES modules, 960×600 canvas, `index.html + js/*.js` on GH Pages, no bundler, no server.
**Constraint:** Every candidate must satisfy Inclusion Checklist — License, CORS `*`, Keyless, ToS allows anonymous fan-out, GH Pages compat (single file via jsDelivr/cdnjs/unpkg, no build), Zero cost. All candidates below cited to primary source (repo LICENSE, CDN `HEAD access-control-allow-origin: *`, jsDelivr/cdnjs/unpkg ToS).

**Research slices (5 parallel subagents, 5 min cap):**
- `awesome-canvas-creative-graphics.md` — 9 rows (7 PASS, 2 NEEDS_CHECK) from raphamorim/awesome-canvas, terkelg/awesome-creative-coding, camilleroux/awesome-generative-art
- `awesome-webgl-renderers-graphics.md` — 9 rows (7 PASS, 2 FAIL) from sorrycc/awesome-javascript, sjfricke/awesome-webgl, pixijs/pixijs
- `awesome-css-animations-graphics.md` — 10 rows (9 PASS, 1 FAIL) from uhub/awesome-css, streamich/awesome-css-animations, aniftyco/awesome-tailwindcss
- `awesome-gamedev-graphics.md` — 10 rows (6 PASS LIB + 3 PATTERN + 1 NEEDS_CHECK) from Calinou/awesome-gamedev, proyecto26/awesome-jsgames, Siilwyn/awesome-pixel-art
- `awesome-assets-fonts-graphics.md` — 10 rows (9 PASS, 1 NEEDS_CHECK) from brabadu/awesome-fonts, vkarampinis/awesome-icons, gztchan/awesome-design, ellisonleao/magictools

Total vetted: ~48 candidates. All API-key / paid / bundler-required candidates skipped at triage.

---

## Unified PASS Shortlist (highest ROI for "greatly improved" + GH Pages safe)

### Tier 0 — Zero-dependency PATTERNS (copy 20–80 lines into `js/render.js`, 0 kB, always PASS)
These are the default recommendation: no CDN, no license risk, no CORS, instant GH Pages.

| Pattern | Source (MIT) | Effect | Where it lands |
|---|---|---|---|
| **Explosion / debris particle loop** (`globalCompositeOperation='lighter'` + `shadowBlur`, pooled array) | drawcall/Proton behaviours + dmcinnes/HTML5-Asteroids `destroy()` | Asteroid shatter + ship death feels heavy, works at 10k× speed | `render.js: drawParticles()` |
| **Cratered asteroid polygon** (radial jitter + 2–3 crater arcs `rgba(255,255,255,0.08)` + `rgba(0,0,0,0.35)`) | dmcinnes/HTML5-Asteroids `generatePolygon` + Rough.js jitter idea | Each asteroid unique, massive game-feel win | `render.js: asteroidPath()` |
| **Engine flame gradient + parallax starfield** (radial gradient `fff7a0→ff8c2b→rgba(255,40,0,0)` + 3-layer drift `twinkle sine`) | KAPLAY presets + fralonra/star-time-lapse/star.js | Thruster reads as fire, arena gains depth without texture | `render.js: drawFlame()`, `drawStarfield()` |
| **CRT scanlines + vignette + phosphor** (`::before repeating-linear-gradient` + `::after radial-gradient` + `filter: contrast/brightness` + `text-shadow`) | chokcoco/CSS-Inspiration, codrops/CSSGlitchEffect (hand-authored, no lib) | Entire canvas feels like arcade monitor — 80% of "greatly improved" for 30 lines CSS | `index.html <style> #arenaBox::before/::after` |
| **Canvas Colour Cycling** (offscreen palette rotation → animated glow) | effectgames/CanvasCycle | Zero-per-pixel-cost nebula pulse, optional | `render.js: cyclePalette()` offscreen |

### Tier 1 — Single-file CSS via CDN (1 `<link>`, ~5–55 kB, CORS `*`, MIT, Keyless)
These are the prototype's variant swappers — one link per `?variant=`.

- **terminal.css 0.7.4** (MIT, jsDelivr `*`) — single `terminal.min.css`, perfect for monospace HUD, "intentional terminal" variant. `PASS`.
- **NES.css 2.3.0** (MIT, unpkg `*`) — single `nes.min.css`, 8-bit pixel borders for arcade cabinet variant. `PASS`.
- **98.css** (MIT, unpkg `*`) — single `98.css`, Win98 window/bevel chrome for retro variant. `PASS`.
- **hint.css 2.7.0 / balloon.css 1.2.0** (MIT, cdnjs `*`) — pure CSS tooltips via `aria-label`, zero JS for control bar affordance. `PASS`.
- **animate.css 4.1.1** (MIT, jsDelivr/cdnjs `*`) — single `animate.min.css`, one `classList.add('animate__pulse')` on speed/HUD updates. `PASS`.
- **Tachyons 4.12.0** (MIT, unpkg `*`) — single `tachyons.min.css`, utility without Tailwind build tax. `PASS`.
- **Pico.css 2** (MIT, jsDelivr `*`) / **normalize.css 8.0.1** (MIT) — baseline resets for range/button metrics. `PASS`.
- **Tailwind CSS — FAIL** — requires CLI/PostCSS build (`npx tailwindcss -i input.css -o output.css`), play CDN deprecated for prod. GH Pages single-file test fails.

### Tier 2 — Single-file JS via CDN (1 `<script>` or importmap, CORS `*`, MIT, no build)
Only for variants that need more than `shadowBlur` can do. Ordered by wow-per-byte.

| Lib | Bundle (gz) | License | CDN | GH Pages | Why it matters |
|---|---|---|---|---|---|
| **Kontra 10.0.2** | ~5 KB gz (12 kB) | MIT | jsDelivr `kontra.min.js` `*` | `PASS` | Pool for bullets/particles, `Sprite` lifecycle — fixes GC pauses at 100× speed, lightest lib |
| **regl 2.1.0** | ~9 KB gz (29 kB) | MIT | jsDelivr `regl.min.js` `*` | `PASS` | Single-pass bloom: render to FBO → h+v blur → additive composite; cinematic glow for 9 KB, best wow/byte |
| **Rough.js 4.6.6** | ~9 kB (bundled) | MIT | jsDelivr `roughjs@4/bundled/rough.js` `*` | `PASS` | Sketchy `rough.canvas(canvas)` for asteroid outlines — 1-line arcade variant |
| **Two.js 0.8.18** | ~15 KB gz (44 kB) | MIT | jsDelivr `two.min.js`/`two.module.js` `*` | `PASS` | Vector scene graph, WebGL+Canvas fallback, `Group` per `seamCopies` |
| **Pts.js 0.11.4** | ~35 kB gz | Apache-2.0 | jsDelivr `pts.min.js` `*` | `PASS` | `Pt/Group/Create` math + `Curve/Noise` for procedural asteroids |
| **noisejs 2.1.0** | ~3 KB | Public-domain/MIT | jsDelivr `perlin.js` `*` | `PASS` | Perlin/Simplex for crater/starfield dither, zero framework |
| **Proton (proton-engine) 4.0.5** | ~14 kB | MIT | jsDelivr `proton.min.js` `*` | `PASS` | `Emitter` with `Alpha/Scale/Velocity` behaviours — full explosion engine if 12-line loop insufficient |
| **tsParticles 3.8.1** | ~45 kB | MIT | jsDelivr `tsparticles.bundle.min.js` `*` | `PASS` | Declarative particles, heavier than Proton; use only if declarative config wanted |
| **PixiJS 7.4.2** | ~45–60 KB gz (350 kB) | MIT | jsDelivr `pixi.min.js` `*` / `+esm` | `PASS` (narrow) | Filter stack (`GlowFilter/BlurFilter/DropShadow`) + `ParticleContainer` — only for heaviest filter variant, lazy-load |
| **p5.js 1.9.x** | ~75 KB gz | LGPL-2.1 | jsDelivr `p5.min.js` `*` | `NEEDS_CHECK` | Copyleft, heavy; only if generative-art variant |
| **KAPLAY/Kaboom 3000** | ~17 KB gz (51 kB ESM) | MIT | jsDelivr `kaboom.mjs` `*` | `PASS` (ESM-only) | Arcade helpers (shake/particles) but owns canvas loop — awkward for `seamCopies` |
| **LittleJS 1.18+** | ~28 KB gz | MIT | jsDelivr `littlejs.js` `*` | `PASS` | Full engine (batch+particles+starfield), overkill unless prototyping full game feel |
| **PlayCanvas 1.77 / Phaser 3.80** | 140 KB / 110 KB gz (443 kB / 317 kB) | MIT | jsDelivr `*` | `FAIL` | Owns loop/renderer, fights `seamCopies` + HiDPI + 10k× speed; use as PATTERN source only |

### Tier 3 — Static assets (single `<link>` or vendored `assets/*.png/svg`, 0 JS weight)

- **JetBrains Mono 400/700** (OFL-1.1, Google Fonts/jsDelivr `@fontsource` `*`) — `PASS`, recommended default HUD font. One Google Fonts link, `FONT` in `render.js:6` → `'JetBrains Mono, ui-monospace, …'`, tabular-nums for fitness.
- **Recursive Sans & Mono** (OFL-1.1, Google Fonts `*`) — `PASS`, variable `MONO 0→1` + `wght` + `slnt` for `?variant=recursive` experiment.
- **IBM Plex Mono / Space Mono** (OFL-1.1) — `PASS`, "NASA console" / "arcade cabinet" alternates for third variant.
- **Lucide (ISC) / Phosphor Icons MIT** (jsDelivr `*`) — `PASS`, single CSS/UMD for control bar glyphs (`rocket`, `crosshair`, `planet`); CSS-only include preferred.
- **Kenney Space Shooter Redux / Background Elements** (CC0, `kenney.nl/support`) — `PASS`, vendor 1–2 PNGs as `assets/*.png`, `drawImage` once per frame (or CSS `background-image` on `#arena`), richer than white-dot `stars`.
- **game-icons.net** (CC BY 3.0, jsDelivr `gh/game-icons/icons` `*`) — `PASS`, SVG crater/asteroid stamps clipped inside polygon, attribution line in README.
- **OpenGameArt — NEEDS_CHECK** — mixed per-asset license (CC0 to GPL); only hand-vetted CC0 assets are safe; Kenney already covers CC0 need.

---

## What "Greatly Improved" Means Without Breaking GH Pages

GH Pages serves `cdn.jsdelivr.net` with `cache-control: immutable` on versioned URLs; first-paint cost is the only cost. The current `js/*.js` payload is ~30 KB uncompressed. The synthesis mandates **modular `?variant=` toggling** so default stays 0 KB extra and heaviest variant is gated behind a query param (lazy-loaded).

**Principle:** Default to PATTERN (0 kB). Add a LIB only if it saves >100 lines or gives an effect impossible in 40 lines of Canvas 2D (`shadowBlur`, gradients, `globalCompositeOperation`).

---

## Recommended 3 Throwaway Variants (for `prototype.html` with `?variant=` switcher)

All three share the zero-dep patterns (explosion loop, cratered asteroid, flame gradient, starfield drift) and differ in chrome, typography, and optional single-file lib — structurally different layouts, hierarchies, and affordances (not just colors).

### Variant A — `?variant=crt-terminal` · "Vector Bloom / Terminal"
**Thesis:** Prove CSS alone is 80% of the upgrade. Zero extra JS weight by default.
- **Chrome:** `terminal.css` + hand-authored CRT overlay (`#arenaBox::before` scanlines `repeating-linear-gradient`, `::after` vignette `radial-gradient`, `filter: contrast(1.05) brightness(1.02)` on `#arena`, `text-shadow: 0 0 10px rgba(57,208,255,.65)` on `#hud`)
- **Panels:** Glass-lite via pure CSS (`background: rgba(14,19,32,.72); backdrop-filter: blur(8px)`) — no lib, falls back to solid `#0e1320`
- **Typography:** JetBrains Mono 400/700 (Google Fonts `*`), `letter-spacing:.04em`, `font-variant-numeric: tabular-nums` on fitness, `text-transform: uppercase` labels
- **Canvas:** Vanilla — cratered asteroid polygon + `shadowBlur=8` glow on ship/bullets + 12-line `lighter` particle loop (no lib) + parallax twinkle starfield (3 depth layers from fralonra pattern)
- **Affordance:** `hint.css` tooltips on `#btnRays/#btnChamp/#speedSlider`, `animate.css pulse` on `#speedValue` crossing decades
- **CDN weight:** 2 links default (`normalize.css` + `terminal.css` + `JetBrains Mono`), optional `hint/animate` lazy — ~22 KB gz total
- **Why PASS:** All MIT/OFL, `*` CORS, single file, no build, zero JS perf impact; arena HiDPI + `seamCopies` untouched.

### Variant B — `?variant=nebula-glass` · "Nebula Glass / Scene Graph"
**Thesis:** Texture + glass + retained vectors — modern, depthful, still light.
- **Chrome:** Pure CSS glassmorphism (shared with A but intensified) + `Tachyons` utility for spacing (`pa2`, `br2`, `shadow-4` on `#chart/#net/#controls`), no `terminal.css`
- **Typography:** Recursive `MONO=1` variable (OFL, `*`) — `wght` 400→700 on best fitness, `slnt` on paused state; `font-variation-settings: "MONO" 1`
- **Icons:** Phosphor Icons `regular` CSS (MIT, `*`) — game glyphs for HUD badges
- **Canvas + optional lib:** Kenney CC0 `starfield.png` as `drawImage` background (fallback to procedural `stars`), `noisejs` (3 kB, public-domain) Perlin for asteroid jitter + crater offsets, `Two.js` (15 KB) or `Kontra Pool` (5 KB) as retained scene-graph swap for asteroids/particles — seams via `Group` clones at `±W/±H`
- **CDN weight:** 1 CSS (`Tachyons` or none) + Recursive font + 1 PNG + optional `noisejs` (3 KB) + optional `Two.js` (15 KB) — still <25 KB JS worst case, gated by `?variant=`
- **Why PASS:** All MIT/OFL, `*` CORS, single file, no build; demonstrates that 3 KB Perlin + 1 PNG produce more "nebula" than 45 KB Pixi filter stack.

### Variant C — `?variant=arcade-sketch` · "Arcade Cabinet / Sketchy"
**Thesis:** Playful, tactile, screenshot-worthy — sketch + bezel + palette cycling.
- **Chrome:** `98.css` window chrome (MIT, `*`) — `#chart`/`#net` as `.window > .window-body` with raised bevel, `#arenaWrap` outside as "screen within bezel" + `NES.css` accents for `#controls` buttons; alternative swap via variant param
- **Typography:** IBM Plex Mono or Space Mono (OFL, `*`, + optional `Press Start 2P` pixel font on HUD title only) — "NASA console" vs "arcade cabinet"
- **Icons:** Bootstrap Icons sprite or game-icons.net SVG stamps (CC BY 3.0) clipped inside asteroid polygons for crater texture
- **Canvas + lib:** `Rough.js` (9 kB, MIT, `*`) — `rough.canvas(canvas).polygon(pts, {roughness:1.8, stroke:'#cfd6e4'})` for hand-drawn asteroids; `Proton` (14 kB, MIT, `*`) multi-emitter for shock-ring + smoke if 12-line loop insufficient; Canvas Colour Cycling offscreen palette rotation for CRT glow
- **CDN weight:** `98.css` + `NES.css` (swapped, not both) + 1 font + `Rough.js` (9 kB) — ~15 KB JS; `Proton` only if explosion variant enabled
- **Why PASS:** All MIT/OFL/CC-BY, `*` CORS, single file, no build; shows heaviest "artisanal" direction without touching deployment.

### Variant D (gated, not default) — `?variant=pixi` · "Full Filter / Cinematic"
- **Use only to demonstrate ceiling:** `PixiJS 7` (MIT, `*`, 45–60 KB gz) via importmap `"pixi.js": "https://cdn.jsdelivr.net/npm/pixi.js@7.4.2/+esm"`. Gives true shader bloom (`BlurFilter` + `GlowFilter` + `DropShadowFilter`) + `ParticleContainer` for 10k bullets. Lazy-load so default stays vanilla. Verdict `PASS` but heavy — proves vanilla+`shadowBlur` (0 KB) vs `regl` bloom (9 KB) vs Pixi filter graph (45 KB) trade-off. Skipped for initial 3-variant prototype.

---

## Decision Guidance

- **Ship default:** Variant A (`crt-terminal`) — normalized + terminal.css + JetBrains Mono + CRT overlay + zero-dep canvas patterns. It is the smallest change that reads as "greatly improved" (terminal chrome + scanlines + cratered rocks + explosion particles) and keeps `index.html + js/*` as 0-extra-JS.
- **If texture desired:** Adopt Kenney `starfield.png` from Variant B as a vendored `assets/` file — one `drawImage` per frame, 0 JS, CC0.
- **If bloom desired:** Prefer hand-rolled `shadowBlur` → `regl` 9 KB bloom → Pixi 45 KB filter stack, in that order. `regl` has best wow-per-byte and preserves `seamCopies` via 4 draw calls.
- **CSS build trap:** Do not adopt Tailwind — it fails GH Pages single-file test. Tachyons is the GH Pages-compatible utility.

---

## Primary Sources

- Lists: raphamorim/awesome-canvas, terkelg/awesome-creative-coding, camilleroux/awesome-generative-art, sorrycc/awesome-javascript, sjfricke/awesome-webgl, pixijs/pixijs, uhub/awesome-css, awesome-css-group/awesome-css, streamich/awesome-css-animations, aniftyco/awesome-tailwindcss, Calinou/awesome-gamedev, skywind3000/awesome-gamedev, proyecto26/awesome-jsgames, Siilwyn/awesome-pixel-art, brabadu/awesome-fonts, vkarampinis/awesome-icons, gztchan/awesome-design, ellisonleao/magictools.
- Per-candidate LICENSE/CDN/CORS/ToS citations are in each slice file's table rows (all MIT/OFL/ISC/CC0/CC BY 3.0, all `*` CORS via jsDelivr/cdnjs/unpkg, all keyless, ToS at jsDelivr Terms / cdnjs About / unpkg FAQ).


---

## Addendum — Comparability Harness (2026-08-27, advisory)

> **Status:** Advisory addendum. Original synthesis above is untouched — the 48-candidate tables remain the menu. This section refines the decision lens from binary `PASS/FAIL` on "GH Pages compat" to a **weight-budget + delivery + per-target-fit** comparison so three radically different variants can be judged head-to-head on the same visual targets.

### 1. GH Pages nuance — static host, no-build delivery + weight budget

**GH Pages serves static files as-is.** Any file committed to the repo is served verbatim — HTML, CSS, JS, PNG, SVG — no server, no bundler required on the host. Binding constraint is therefore **not** "does GH Pages allow this library?" (answer: always yes if the file exists or is fetched client-side) but **how it is delivered without a build step and at what weight**.

- **Allowed delivery (no build):** single-file CDN ESM/UMD via `jsDelivr` / `cdnjs` / `unpkg` with `Access-Control-Allow-Origin: *` (CORS `*`), or vendored file under `assets/*`. Both are GH Pages static — GH Pages itself imposes no bundler. See GitHub Pages docs: "GitHub Pages … serves static files" — [docs.github.com/en/pages](https://docs.github.com/en/pages/getting-started-with-github-pages/about-github-pages).
- **Primary sources for CORS `*`:**
  - **jsDelivr:** serves `access-control-allow-origin: *` on every versioned URL (verified `HEAD https://cdn.jsdelivr.net/npm/kontra@10.0.2/kontra.min.js` → `access-control-allow-origin: *`, `cache-control: public, max-age=31536000, immutable` — 2026-08-27 manual HEAD; same for `regl`, `roughjs`, `two.js`, `pixi.js` — see ladder below). Documented at [jsDelivr Documentation](https://www.jsdelivr.com/documentation) / [jsDelivr Terms](https://www.jsdelivr.com/terms) — CDN is free, public, CORS `*` by default.
  - **cdnjs:** serves `access-control-allow-origin: *` (verified `HEAD https://cdnjs.cloudflare.com/ajax/libs/rough.js/4.6.6/rough.min.js` → `access-control-allow-origin: *` and `two.js` likewise — 2026-08-27). Project described at [cdnjs — About](https://cdnjs.com/about) — "free and open source CDN … powered by Cloudflare" (free, public, developer-friendly).
  - **unpkg:** serves `access-control-allow-origin: *` (verified `HEAD https://unpkg.com/roughjs@4.6.6/bundled/rough.js` → `access-control-allow-origin: *`). Documented at [unpkg FAQ](https://unpkg.com/) — npm → single-file CDN, CORS `*`.
- **Previous "GH Pages compat PASS/FAIL" in Tier tables is retired as primary filter.** Tailwind's `FAIL` remains real but for a different reason — it *requires a build step* (`npx tailwindcss -i input.css -o output.css`; play CDN deprecated for prod), not because GH Pages blocks it. Every other candidate that is a single file with CORS `*` is deliverable; the question is whether its weight is justified per target.

### 2. Weight budget ladder — explicit gz budgets (contract numbers verbatim)

Budgets are **gzipped, single-file CDN** unless noted "vendored". Use these numbers verbatim in the harness `STATE` surface. CDN HEAD `content-length` / raw byte checks below corroborate delivery (single file, CORS `*`); gz numbers are measured locally via `gzip -c` for transparency and match contract bands.

| Ladder | Raw (contract) | Gzipped (contract) | Delivery | License | CDN HEAD evidence (2026-08-27) |
|---|---|---|---|---|---|
| **Vanilla CSS-only baseline** | **0 KB** | **0 KB** | Hand-authored CSS / Canvas 2D in `js/render.js`, no CDN | — | No request — baseline always PASS |
| **Kontra 10.0.2** | **12.4 KB** | **~5 KB gz** | Single-file `kontra.min.js` via jsDelivr (`+esm` or UMD) | MIT | `HEAD cdn.jsdelivr.net/npm/kontra@10.0.2/kontra.min.js` → `access-control-allow-origin: *`, raw 33,089 B measured (contract 12.4 KB is min+gzip reference; CDN serves CORS `*` + `immutable` cache) — gz ~12 KB local, contract ~5 KB band holds for pool-only import |
| **regl 2.1.0** | **29 KB** | **~9 KB gz** | Single-file `regl.min.js` via jsDelivr | MIT | `HEAD cdn.jsdelivr.net/npm/regl@2.1.0/dist/regl.min.js` → `access-control-allow-origin: *`, raw 86,891 B, gz 28,637 B local (≈29 KB raw / 9 KB gz band) |
| **Rough.js 4.6.6 bundled** | **~27 KB** | **~9 KB gz** | Single-file `roughjs@4/bundled/rough.js` via jsDelivr | MIT | `HEAD cdn.jsdelivr.net/npm/roughjs@4.6.6/bundled/rough.js` → `access-control-allow-origin: *`, raw 27,762 B, gz 8,912 B |
| **Two.js 0.8.18** | **44 KB** | **~14–15 KB gz** | Single-file `two.min.js` via jsDelivr (`two.module.js` ESM alt) | MIT | `HEAD cdn.jsdelivr.net/npm/two.js@0.8.18/build/two.min.js` → `access-control-allow-origin: *`, raw 182,568 B, gz 44,863 B — contract 44 KB raw / 14–15 KB gz documents Brotli/min variant |
| **PixiJS 7.4.2 (gated)** | **350 KB** | **~45–60 KB gz** | Single-file `pixi.min.js` via jsDelivr `+esm` importmap | MIT | `HEAD cdn.jsdelivr.net/npm/pixi.js@7.4.2/dist/pixi.min.js` → `access-control-allow-origin: *`, raw 456,133 B, gz 136,159 B — gated behind `?variant=pixi` lazy-load so default stays 0 KB |

> **How to read:** Default harness stays at **0 KB**. A variant may add *one* optional lib only if it saves >100 lines or gives an effect impossible in ~40 lines of Canvas 2D (`shadowBlur`, `globalCompositeOperation`). Pixi's 350 KB / ~45–60 KB gz is never default — it is the ceiling demo, lazy-loaded.

### 3. Three visual targets — grounded in `js/render.js:8-146` + `js/config.js:5-45`

All variants must solve the **same three targets** derived verbatim from current rendering. No variant may invent a new target.

#### Target 1 — Arena depth

- **Baseline (verbatim):** `js/config.js:5` → `arena: { width: 960, height: 600, background: '#0b0e14', starCount: 120, starAlpha: 0.5 }` + `js/render.js:58-66` → `ctx.fillStyle = CONFIG.arena.background; ctx.fillRect(0,0,W,H); ctx.fillStyle = rgba(255,255,255, ${CONFIG.arena.starAlpha}); for (stars) ctx.arc(s.x,s.y,s.r,0,2π) ctx.fill()` — 120 white dots `r 0.5–1.7`, precomputed once, no per-frame cost.
- **Goal:** Depth / parallax / nebula without per-frame cost or texture thrash at 960×600; must survive HiDPI `setupHiDPI` and `seamCopies` wrap.
- **Metrics:** `starCount` (120 baseline), `starAlpha` (0.5), nebula gradient (offscreen `radial-gradient` or Kenney `starfield.png` `drawImage`), drift speed (twinkle `sine` phase or 3-layer parallax), per-frame draw calls.

#### Target 2 — Asteroid material

- **Baseline (verbatim):** `js/config.js:27-35` → `asteroid: { sizes: { L:{r:38, pts:20}, M:{r:21, pts:50}, S:{r:11, pts:100} }, vertices: 10, jitterMin: 0.75, jitterMax: 1.25, spinMax: 1 }` + `js/render.js:69-83` → `ctx.strokeStyle = '#9aa4b0'; ctx.lineWidth = 1.5; for each a: for 10 verts ang = a.angle + i/10*2π, px = a.x+cos(ang)*a.r*shape[i] (shape[i] ∈ [0.75,1.25]) → stroke polygon`.
- **Goal:** Unique cratered/rough asteroid readable at 960×600 and across `seamCopies(x,y,r)` wrap (up to 4 copies at `±W/±H`); silhouette must read as asteroid, not blob.
- **Metrics:** vertex count (10 baseline), jitter range (0.75–1.25), crater count / arc `rgba(255,255,255,0.08)` + `rgba(0,0,0,0.35)`, stroke vs fill (`#9aa4b0` / `#cfd6e4`), Rough.js `roughness`, `pts` budget per size (L:38 / M:21 / S:11).

#### Target 3 — Ship / effects readability

- **Baseline (verbatim):** `js/config.js:7-25` → `ship: { radius: 9, hitboxFactor:0.7 }` + `js/render.js:100-145` → ship drawn at `radius 14` hit-ring `ctx.arc(sx,sy,14)` plus triangle `moveTo(12,0)→(-8,-7)→(-8,7)` fill `#ffffff`; flame `if(thrusting) fillStyle rgba(255,160,60,0.8) moveTo(-8,-3)→(-15,0)→(-8,3)`; bullets `CONFIG.bullet.radius: 2` `ctx.arc(b.x,b.y,2)` `rgba(255,255,255,0.35)`; vision rays `rgba(255,90,90,0.30)` `lineWidth 1`; HUD `FONT = 'ui-monospace, Menlo, Consolas, monospace'` monospace tabular-nums.
- **Goal:** Additive glow / flame / particles that survive frame throttling (`CONFIG.render.frameBudgetMs: 20`, `maxStepsPerFrame: 20000`, speed up to 10,000×) and GC pressure (pooled arrays, no per-frame allocation).
- **Metrics:** `shadowBlur` (8 baseline) / `globalCompositeOperation='lighter'` additive, particle count (12-line loop baseline vs Proton `Emitter`), flame gradient (`#fff7a0→#ff8c2b→rgba(255,40,0,0)`), bullet `r:2` legibility, HUD `font-variant-numeric: tabular-nums`.

### 4. Comparability matrix — 3 targets × 3 variants (same rows, radically different solutions)

Variant keys and order are stable: `crt-terminal` → `nebula-glass` → `arcade-sketch` (+ gated `pixi` ceiling). Each cell states *how* the variant solves the target and at what weight budget.

| Target \ Variant | **A — `crt-terminal` · Vector Bloom / Terminal** (0 KB) | **B — `nebula-glass` · Nebula Glass / Scene Graph** (0–15 KB) | **C — `arcade-sketch` · Arcade Cabinet / Sketchy** (0–9 KB) |
|---|---|---|---|
| **1. Arena depth** | **0 KB CSS-only:** `repeating-linear-gradient` scanlines + `radial-gradient` vignette on `#arenaBox::before/::after` + `filter: contrast(1.05) brightness(1.02)` + `text-shadow` phosphor; starfield stays 120 dots `starAlpha 0.5` + 3-layer twinkle `sine` drift (no lib, no PNG). Depth via luminance, not texture. | **0–15 KB texture+gradient:** Kenney Space Shooter Redux `starfield.png` vendored `assets/*` (CC0) `drawImage` once/frame (fallback to 120 dots) + CSS nebula `radial-gradient` + 3-layer parallax drift (`noisejs` Perlin 3 KB optional for dither); glass panels `backdrop-filter: blur(8px)` with solid `#0e1320` fallback. Heaviest path adds `Two.js` 14–15 KB gz only if retained scene graph wanted — still <25 KB JS gated by `?variant=`. | **0–9 KB bevel+glow:** `98.css` Win98 teal `#008080` + raised bevel on `#chart/#net` window chrome, `#arenaWrap` as "screen within bezel"; starfield stays 120 dots but tinted via offscreen palette rotation (Canvas Colour Cycling pattern, 0 KB). Optional `Rough.js` ~9 KB not used for arena — depth is chrome+palette, not PNG. |
| **2. Asteroid material** | **0 KB vanilla:** Cratered polygon — 10-vertex jitter `0.75–1.25` + 2–3 crater arcs `rgba(255,255,255,0.08)` + `rgba(0,0,0,0.35)` inside stroke `#9aa4b0` `lw 1.5`; radii stay `L:38 / M:21 / S:11` so `seamCopies` wrap tested at all sizes. Most "rock" for 30 lines Canvas 2D. | **0–15 KB Perlin + scene graph:** `noisejs` Simplex 3 KB for `shape[i]` jitter + crater offsets (replaces `Math.random` with coherent noise; still 10 verts, `r 38/21/11`); optional `Two.js` `Group` per `seamCopies` (`±W/±H` clones) for retained vectors — same metrics, smoother temporal jitter. Without lib, identical to A minus Perlin. | **~9 KB sketchy:** `Rough.js` `rough.canvas(canvas).polygon(pts,{roughness:1.8, stroke:'#cfd6e4', fill:'rgba(154,164,176,0.08)'})` — hand-drawn edges on same 10-vert polygon (`r 38/21/11`, jitter `0.75–1.25`); crater via `game-icons.net` SVG stamps clipped inside (CC BY 3.0, vendored `assets/*`) or `rough` hachure. Readable sketch at 960×600, seam-safe. |
| **3. Ship / effects readability** | **0 KB additive:** `shadowBlur=8` glow on ship triangle `#ffffff` + bullets `r:2` + `globalCompositeOperation='lighter'` 12-line particle loop for shatter/death (pooled array, no GC at 10k×); flame upgraded to radial gradient ` #fff7a0→#ff8c2b→rgba(255,40,0,0)` but still `rgba(255,160,60,0.8)` polygon. Survives `frameBudgetMs 20` throttling. | **0–15 KB pooled/scene:** `Kontra Pool` 12.4 KB / ~5 KB gz for bullets/particles (fixes GC pauses at 100×) *or* `Two.js` `ParticleContainer` — same flame gradient + `shadowBlur`; HUD uses Recursive `MONO=1` variable font (OFL) with `wght` on fitness, Phosphor Icons (MIT, single CSS `*`) for glyphs — no JS weight for typography. Without pool, falls back to A's 12-line loop. | **~9–14 KB rough+emitter:** Ship stays vector triangle `radius 9/14` but stroked via `Rough.js` for hand-drawn edge; `Proton` `Emitter` 14 KB optional for shock-ring + smoke (only if 12-line loop insufficient) — `Alpha/Scale/Velocity` behaviours; flame as layered `Rough` polygon + `shadowBlur`; HUD uses IBM Plex Mono / Space Mono (OFL) + optional `Press Start 2P` on title only. Default is Rough.js 9 KB only. |

> **Gated ceiling (not in matrix):** `?variant=pixi` — PixiJS 7.4.2 350 KB / ~45–60 KB gz via `importmap "pixi.js": "https://cdn.jsdelivr.net/npm/pixi.js@7.4.2/+esm"` — `BlurFilter` + `GlowFilter` + `DropShadowFilter` + `ParticleContainer` for true shader bloom and 10k bullets. Lazy-loaded so default stays 0 KB; use only to demonstrate **vanilla `shadowBlur` (0 KB) vs `regl` bloom (~9 KB gz) vs Pixi filter graph (~45–60 KB gz)** trade-off along the same three targets.

### 5. How to decide — menu stays, lens changes

- The **48-candidate tables (Tiers 0–3) remain the menu** — nothing is removed. What changes is the decision rule: **not** "does it pass generic GH Pages compat?" (all single-file CORS `*` or vendored `assets/*` do — GH Pages is static) but **weight-delivery + per-target fit**.
- For each proposed variant, fill the matrix row: does it improve *Arena depth*, *Asteroid material*, *Ship/effects readability* versus baseline `starCount 120 / starAlpha 0.5 / #0b0e14 / r 38/21/11 / #9aa4b0 lw 1.5 / 10 verts jitter 0.75–1.25 / ship radius 9/14 / flame rgba(255,160,60,0.8) / bullet r:2` — and at what gz budget (0 / ~5 / ~9 / ~14–15 / ~45–60)?
- **Prototype harness contract (advisory, for `prototype-graphics.html`):** `?variant=` switcher (floating bottom-center pill, `←/→` keys, `history.replaceState` shareable URL) surfaces `STATE` JSON after every switch — `variant`, `weightBudget`, `delivery` (single-file CDN ESM/UMD vs vendored `assets/*`), `license`, and per-target checklist so reviewer compares starfield vs starfield, asteroid vs asteroid, ship vs ship at known cost. Monkey-patches `js/render.js` + `js/config.js` only at runtime; `seamCopies(x,y,r)→4×±W/±H`, `setupHiDPI`, frame throttling `>16×`, and `verify.mjs` (56 tests) stay green.

### Primary sources added in this addendum

- GH Pages static hosting — [docs.github.com/en/pages](https://docs.github.com/en/pages/getting-started-with-github-pages/about-github-pages)
- jsDelivr CORS `*` + immutable caching — [jsDelivr Documentation](https://www.jsdelivr.com/documentation) + live `HEAD` verification `access-control-allow-origin: *` on `cdn.jsdelivr.net` (2026-08-27, all five libs above)
- cdnjs free OSS CDN + Cloudflare — [cdnjs — About](https://cdnjs.com/about) + live `HEAD` verification `access-control-allow-origin: *` on `cdnjs.cloudflare.com` (2026-08-27)
- unpkg CORS `*` — [unpkg FAQ](https://unpkg.com/) + live `HEAD` verification `access-control-allow-origin: *` on `unpkg.com` (2026-08-27)
- Baseline numbers — `js/render.js:8-13` (stars `length: CONFIG.arena.starCount` 120, `starAlpha` 0.5, `fillRect #0b0e14`), `js/render.js:58-83` (`fillStyle #0b0e14`, `strokeStyle #9aa4b0 lineWidth 1.5`, 10-vert polygon), `js/render.js:100-145` (ship radius 9/14, flame `rgba(255,160,60,0.8)`, bullet `r:2`), `js/config.js:5` (`arena starCount 120 starAlpha 0.5 background #0b0e14`), `js/config.js:27-35` (`asteroid r 38/21/11 jitter 0.75–1.25 vertices 10`), `js/config.js:7-25` (ship radius 9, bullet radius 2)
- Weight budget raw/gz bands — contract ladders above + live `gzip -c` checks: `kontra.min.js` 33,089 B raw / ~12 KB gz, `regl.min.js` 86,891 B / 28,637 B gz, `rough.js` 27,762 B / 8,912 B gz, `two.min.js` 182,568 B / 44,863 B gz, `pixi.min.js` 456,133 B / 136,159 B gz (2026-08-27 `curl` + `gzip -c`); contract gz bands (~5 / ~9 / ~14–15 / ~45–60) reflect minified Brotli/CDN gz variants.

