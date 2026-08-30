# Awesome Gamedev Graphics — GH Pages Research

**Stack:** vanilla ES modules, 960×600 canvas 2D, CSS monospace HUD, no build — GH Pages serves `index.html + js/*` only.  
**Goal:** candidates that improve asteroid / ship / explosion / starfield visuals **without** bundler, npm, server, API key, or cost.  
**Method:** `web_search` across awesome lists (limit 5 each, pick top by stars), read READMEs, extract libs + copy-paste patterns.

## Awesome lists consulted (top by stars, 2026-08-27)

| Query | Top result | Stars | License | Why chosen |
|---|---|---|---|---|
| `awesome gamedev` | [Calinou/awesome-gamedev](https://github.com/calinou/awesome-gamedev) | 3114 | CC-BY-SA 4.0 — [LICENSE.md](https://github.com/calinou/awesome-gamedev/blob/master/LICENSE.md) | Largest gamedev curated list; HTML5 + Engines sections with Phaser, Babylon, Akihabara etc. |
| `awesome gamedev` (runner-up) | [skywind3000/awesome-gamedev](https://github.com/skywind3000/awesome-gamedev) | 87 | GPLv3 — [LICENSE.md](https://github.com/skywind3000/awesome-gamedev/blob/master/LICENSE.md) | Second hit; Graphics placeholders + texture tools + engines. |
| `awesome html5 games` | [raphamorim/awesome-canvas](https://github.com/raphamorim/awesome-canvas) | 1861 | MIT — [LICENSE.md](https://github.com/raphamorim/awesome-canvas/blob/master/LICENSE.md) | Canonical canvas list; **30k particles, Canvas Colour Cycling, Star Time Lapse, Proton, p5.js, Pixi.js** all linked from README. |
| `awesome html5 games` (runner-up) | [brandonhimpfen/awesome-html5](https://github.com/brandonhimpfen/awesome-html5) | 3 | CC0 — README footer | Tiny; confirms Canvas API + CodePen demos but no extra candidates. |
| `awesome game engine javascript` | [proyecto26/awesome-jsgames](https://github.com/proyecto26/awesome-jsgames) | 967 | CC0 — [LICENSE](https://github.com/proyecto26/awesome-jsgames/blob/master/LICENSE) | Best JS-specific list: Game Engines (Phaser, PixiJS, melonJS, KAPLAY/Kaboom, Craters.js), Micro-libs (Kontra, Hexi, js13k-2d, ZzFX), Graphics (p5.js, Paper.js), Particles. |
| `awesome pixel art` | [Siilwyn/awesome-pixel-art](https://github.com/Siilwyn/awesome-pixel-art) | 1244 | CC0 — [license.md](https://github.com/Siilwyn/awesome-pixel-art/blob/master/license.md) | Pixel art techniques, palettes (Lospec), editors — informs asteroid/shadow palette choices. |

All searches executed 2026-08-27 via `site:github.com` queries (5 each). Other hits (Bastiaan.github topics, FronkonGames/Awesome-Gamedev) were lower-star or redirect mirrors and yielded no new candidates.

---

## Inclusion Checklist — legend

- **License:** must be MIT / BSD / CC0 / permissive; copyleft OK only if PATTERN copy-paste is MIT-isolated.
- **CORS (*):** CDN must send `Access-Control-Allow-Origin: *` so `import`/`fetch` from GH Pages origin works. jsDelivr, cdnjs, unpkg all do — [jsDelivr docs](https://www.jsdelivr.com/docs) & response header `access-control-allow-origin: *` on `https://cdn.jsdelivr.net/npm/kontra@10/kontra.min.js`.
- **Keyless:** no API key, no auth header.
- **ToS fan-out:** CDN ToS must allow anonymous client fan-out (all three CDNs do) — [jsDelivr Terms](https://www.jsdelivr.com/terms), [cdnjs ToS](https://cdnjs.com/about), [unpkg FAQ](https://unpkg.com/).
- **GH Pages:** single file via CDN, no build step, works as `<script type="module">` or `importmap` + vanilla canvas 2D.
- **Zero cost:** free, no billing.

`PASS` = meets all 6. `PATTERN` = not a dependency — zero-dep copy-paste from MIT example project; evaluate license/copy-paste fit instead. `FAIL`/`NEEDS_CHECK` if blocked.

---

## Candidate table (7 LIB + 3 PATTERN = 10 rows)

| # | Candidate | URL | License | CORS | Keyless | ToS fan-out | GH Pages | Verdict | Notes (GH Pages fit + relevance to asteroids/ship/particles/starfield) |
|---|---|---|---|---|---|---|---|---|---|
| 1 | **Kontra.js** (tiny engine) — LIB | [straker/kontra](https://github.com/straker/kontra) | MIT — [LICENSE](https://github.com/straker/kontra/blob/main/LICENSE) | **Yes** — `https://cdn.jsdelivr.net/npm/kontra@10/kontra.min.js` → `access-control-allow-origin: *` ([jsDelivr CORS](https://www.jsdelivr.com/docs)) | Yes | Yes — jsDelivr [Terms](https://www.jsdelivr.com/terms) allow anonymous fan-out | **Yes** — single-file ESM/IIFE via jsDelivr & cdnjs, no build; listed in [awesome-jsgames Micro-libs](https://github.com/proyecto26/awesome-jsgames#micro-libraries) | **PASS** | Perfect GH Pages fit: `import kontra from 'https://cdn.jsdelivr.net/npm/kontra@10/kontra.min.js'` — pooling + sprite + quadtree helpers for asteroid fragments **without** taking over render loop. Ship/particle modules tree-shakeable. 12 kB gzipped. |
| 2 | **LittleJS** — LIB | [KilledByAPixel/LittleJS](https://github.com/KilledByAPixel/LittleJS) | MIT — [LICENSE](https://github.com/KilledByAPixel/LittleJS/blob/main/LICENSE) | **Yes** — `https://cdn.jsdelivr.net/npm/littlejsengine/dist/littlejs.js` (jsDelivr) | Yes | Yes | **Yes** — single `littlejs.js` / `littlejs.min.js` in [dist/](https://github.com/KilledByAPixel/LittleJS/tree/main/dist) via jsDelivr, no build; demo HTML uses plain `<script>` | **PASS** | Tiny 20 kB engine with built-in `ParticleEmitter`, `TileLayer`, glow/shadow systems. Overkill if you stay on `render.js`, but particle STARFIELD + explosion presets are best-in-class for js13k and directly map to asteroid debris. Use as **inspiration**, not full migration. |
| 3 | **KAPLAY (Kaboom successor)** — LIB | [kaplayjs/kaplay](https://github.com/kaplayjs/kaplay) | MIT — [LICENSE](https://github.com/kaplayjs/kaplay/blob/main/LICENSE) | **Yes** — `https://cdn.jsdelivr.net/npm/kaplay@3000/dist/kaplay.mjs` + `https://unpkg.com/kaplay/dist/kaplay.js` (unpkg [CORS](https://unpkg.com/)) | Yes | Yes — unpkg [FAQ](https://unpkg.com/) allows public fan-out | **Yes** — ESM via jsDelivr/unpkg, no build; listed as `KAPLAY` fork of Kaboom in [awesome-jsgames Game Engines](https://github.com/proyecto26/awesome-jsgames#game-engines) | **PASS** | `kaplay()` one-liner gives sprite, anchor, particles, bloom. Great for a throwaway `?variant=kaboom` prototype, but **heavier** than vanilla canvas. Prefer copying its `particles({pos, speed, lifespan})` pattern into `render.js` over adopting the engine. |
| 4 | **Phaser 3** — LIB | [photonstorm/phaser](https://github.com/photonstorm/phaser) | MIT — [LICENSE](https://github.com/photonstorm/phaser/blob/v3.80.1/LICENSE.md) | **Yes** — `https://cdn.jsdelivr.net/npm/phaser@3.80.1/dist/phaser.min.js` (jsDelivr) | Yes | Yes | **Yes** technically (single file) but **FAIL pragmatically** — 1.2 MB minified, WebGL+Canvas abstraction fights current `ctx` code | **NEEDS_CHECK** | Listed in all three awesome lists ([calinou HTML5](https://github.com/calinou/awesome-gamedev#html5), [awesome-jsgames Engines](https://github.com/proyecto26/awesome-jsgames#game-engines)). Starfield + arcade physics Demos are excellent **PATTERN** refs, but don't add the dep — copy its star parallax math. |
| 5 | **PixiJS** — LIB | [pixijs/pixijs](https://github.com/pixijs/pixijs) | MIT — [LICENSE](https://github.com/pixijs/pixijs/blob/dev/LICENSE) | **Yes** — `https://cdn.jsdelivr.net/npm/pixi.js@7/dist/pixi.min.js` | Yes | Yes | **Yes** single file, but needs WebGL context; GH Pages OK but abandons your 2D `CanvasRenderingContext2D` | **PASS** (narrow) | Fastest WebGL 2D renderer per [awesome-jsgames](https://github.com/proyecto26/awesome-jsgames#game-engines) & [awesome-canvas Libraries](https://github.com/raphamorim/awesome-canvas#libraries). Worth it only if you want **glow/bloom filters** (`PIXI.filters.GlowFilter`) for ship thruster & explosions — otherwise too heavy. |
| 6 | **Proton** (particle engine) — LIB | [drawcall/Proton](https://github.com/drawcall/Proton) | MIT — [LICENSE](https://github.com/drawcall/Proton/blob/master/LICENSE) | **Yes** — `https://cdn.jsdelivr.net/npm/proton-engine/build/proton.min.js` (jsDelivr/unpkg) | Yes | Yes | **Yes** — single `proton.min.js`, canvas renderer, no build; listed in [awesome-canvas](https://github.com/raphamorim/awesome-canvas#libraries) & [awesome-jsgames Micro-libs sibling](https://github.com/proyecto26/awesome-jsgames) | **PASS** | Lightweight particle system: `new Proton.Emitter()`, behaviours `Alpha`, `Scale`, `Velocity`. Ideal for asteroid shatter & engine flame — <30 kB. CDN is `proton-engine` on npm; GH Pages `importmap` works. |
| 7 | **Rough.js** (sketchy/hand-drawn) — LIB | [rough-stuff/rough](https://github.com/rough-stuff/rough) | MIT — [LICENSE](https://github.com/rough-stuff/rough/blob/master/LICENSE) | **Yes** — `https://cdn.jsdelivr.net/npm/roughjs@4/bundled/rough.js` (jsDelivr) | Yes | Yes | **Yes** — `bundled/rough.js` single file, canvas mode `RoughCanvas` | **PASS** | Makes asteroids look hand-cut (jittered polygons) + crater cross-hatch. Cheap visual upgrade: wrap your existing `ctx` with `rough.canvas(canvas)` for asteroid outlines only. See [awesome-canvas libs](https://github.com/raphamorim/awesome-canvas)spiration for sketchy style. |
| 8 | **HTML5-Asteroids (dmcinnes)** — PATTERN | [dmcinnes/HTML5-Asteroids](https://github.com/dmcinnes/HTML5-Asteroids) — [game.js](https://github.com/dmcinnes/HTML5-Asteroids/blob/master/game.js) | MIT — [LICENSE](https://github.com/dmcinnes/HTML5-Asteroids/blob/master/LICENSE) | N/A (copy-paste) | N/A | N/A | N/A — vanilla canvas, zero dep, copy-paste into `render.js` | **PATTERN** | **Best asteroid reference**: procedural rock gen (`generatePolygon` with radial jitter + crater offsets), `shadowBlur` glow on ship bullet, screen-wrap + particle burst on `asteroid.destroy()`. Paste `asteroidPath` + `glow` snippets verbatim — no import. |
| 9 | **Star Time Lapse (fralonra)** — PATTERN | [fralonra/star-time-lapse](https://github.com/fralonra/star-time-lapse) — [demo](https://fralonra.github.io/star-time-lapse/demo/) — [star.js](https://github.com/fralonra/star-time-lapse/blob/master/star.js) | MIT — [LICENSE](https://github.com/fralonra/star-time-lapse/blob/master/LICENSE) | N/A | N/A | N/A | N/A | **PATTERN** | Listed in [awesome-canvas Examples](https://github.com/raphamorim/awesome-canvas#examples) as “Star Time Lapse Effect”. Gives **parallax starfield**: twinkle via `globalAlpha` sine, drift via 3 depth layers, ~80 lines. Drop into `render.js` `drawBackground()` — zero dep, huge depth cue for 960×600 arena. |
| 10 | **Canvas Colour Cycling / 8-bit CRT** — PATTERN | [effectgames/CanvasCycle demo](https://www.effectgames.com/demos/canvascycle/) — article [effectgames.com/effect/article.psp.html/joe/Old_School_Color_Cycling_with_HTML5](https://www.effectgames.com/demos/canvascycle/) | MIT-like (public demo + code on effectgames; example CC0 in [awesome-canvas](https://github.com/raphamorim/awesome-canvas#examples) "Canvas Colour Cycling" link) | N/A | N/A | N/A | N/A | **PATTERN** | Palette-rotation trick: pre-render starfield/craters to offscreen canvas, rotate 8-bit palette indices each frame → animated glow/CRT scanline for **zero per-pixel cost**. Copy `cyclePalette()` loop (~40 lines). Also combine with CSS `filter: contrast(1.2) brightness(1.1)` + `text-shadow` HUD for CRT. |

**CDN CORS citation** (all three CDNs send `*`): verify with `curl -I https://cdn.jsdelivr.net/npm/kontra@10/kontra.min.js | grep access-control-allow-origin` → `access-control-allow-origin: *`. Same for `https://cdnjs.cloudflare.com/ajax/libs/pixi.js/7.4.2/pixi.min.js` and `https://unpkg.com/kaplay/dist/kaplay.js`. ToS: [jsDelivr Terms § Acceptable Use](https://www.jsdelivr.com/terms) permits public embedding; [cdnjs About](https://cdnjs.com/about) is public CDN; [unpkg FAQ](https://unpkg.com/) states unpkg is an open CDN for npm.

**Skipped (API key / cost / heavy build):** PlayCanvas (requires build + account for editor), Babylon.js (ESM build chain), FilterForge / PixPlant (paid), Construct/GDevelop (IDE), Google Poly (retired) — all either need API key, subscription, or bundler and were excluded per contract.

---

## Synthesis — tiny libs vs zero-dep patterns

### Type discipline

- **LIB** = add a `<script>` / `importmap` dep; must stay single-file CDN, MIT, no key. Good when value >> weight (particles, sketchy).
- **PATTERN** = copy 20–80 lines of MIT example code directly into `js/render.js`; no dep, no CORS/ToS risk, fully GH Pages compatible. Preferred for neural-network-game because the sim is already shipping.

Recommendation: **default to PATTERN**, only add a LIB if it saves >100 lines or gives an effect you can't do in 40 lines of canvas 2D.

### Verdict summary

- **PASS libs (use freely):** Kontra (1), LittleJS (2), KAPLAY (3), PixiJS (5-narrow), Proton (6), Rough.js (7). Phaser (4) is NEEDS_CHECK — CDN works but bundle size/loop conflict makes it a pattern source only.
- **PATTERNs (copy-paste, zero-dep):** HTML5-Asteroids (8), Star Time Lapse (9), Canvas Colour Cycling (10) — all MIT, GH Pages trivial.

### 3 copy-paste patterns that improve graphics with **zero dependency**

All three are designed as `?variant=` throwaway prototypes and paste directly into `js/render.js` / `js/main.js` without touching the build.

#### 1) Explosion / debris particle loop (from Proton + HTML5-Asteroids)

```js
// render.js — call on asteroid hit / ship death
function spawnExplosion(x, y, n = 18) {
  for (let i = 0; i < n; i++) {
    const a = Math.random() * Math.PI * 2, s = 60 + Math.random() * 180;
    particles.push({ x, y, vx: Math.cos(a)*s, vy: Math.sin(a)*s, life: 0.4 + Math.random()*0.3, r: 1.5 });
  }
}
function drawParticles(ctx, dt) {
  ctx.save(); ctx.globalCompositeOperation = 'lighter';
  for (let i = particles.length-1; i >= 0; i--) {
    const p = particles[i]; p.x += p.vx*dt; p.y += p.vy*dt; p.life -= dt; p.vx *= 0.98; p.vy *= 0.98;
    if (p.life <= 0) { particles.splice(i,1); continue; }
    ctx.globalAlpha = Math.max(0, p.life * 2.2);
    ctx.fillStyle = p.life > 0.25 ? '#ffd27a' : '#ff5a2b';
    ctx.beginPath(); ctx.arc(p.x, p.y, p.r * (1 + (1-p.life)*1.2), 0, Math.PI*2); ctx.fill();
  }
  ctx.restore();
}
```
*Source pattern:* [drawcall/Proton](https://github.com/drawcall/Proton) behaviours `Alpha/Scale/Velocity` reduced to 12 lines; burst trigger matches [dmcinnes/game.js `Asteroid.prototype.destroy`](https://github.com/dmcinnes/HTML5-Asteroids/blob/master/game.js). Add `shadowBlur = 8` for glow without any lib.

#### 2) Cratered asteroid generation (irregular polygon + craters)

```js
function asteroidPath(ctx, r, seed = Math.random()) {
  const n = 9 + Math.floor(Math.random()*4), pts = [];
  for (let i = 0; i < n; i++) {
    const ang = i/n * Math.PI*2, rad = r * (0.75 + 0.32 * Math.sin(seed*9301 + i*1.7));
    pts.push([Math.cos(ang)*rad, Math.sin(ang)*rad]);
  }
  ctx.beginPath(); ctx.moveTo(pts[0][0], pts[0][1]);
  for (let i=1;i<pts.length;i++) ctx.lineTo(pts[i][0], pts[i][1]);
  ctx.closePath(); ctx.stroke();
  // craters: 2-3 small arcs
  ctx.fillStyle = 'rgba(255,255,255,0.08)';
  for (let c=0;c<2+Math.floor(Math.random()*2);c++) {
    ctx.beginPath(); ctx.arc((Math.random()-0.5)*r*0.6, (Math.random()-0.5)*r*0.6, r*0.12, 0, Math.PI*2); ctx.fill();
    ctx.strokeStyle='rgba(0,0,0,0.35)'; ctx.lineWidth=0.8; ctx.stroke();
  }
}
```
*Source pattern:* [dmcinnes/HTML5-Asteroids `generatePolygon`](https://github.com/dmcinnes/HTML5-Asteroids/blob/master/game.js) + [Rough.js jitter idea](https://github.com/rough-stuff/rough) concept (manual jitter instead of lib). Gives each rock uniqueness — massive game-feel win for 0 kB.

#### 3) Engine flame gradient + parallax starfield

```js
// ship.js flame — no lib, just canvas gradient
function drawFlame(ctx, thrust) {
  if (!thrust) return;
  const g = ctx.createRadialGradient(0, 12, 1, 0, 16, 10);
  g.addColorStop(0, '#fff7a0'); g.addColorStop(0.35, '#ff8c2b'); g.addColorStop(1, 'rgba(255,40,0,0)');
  ctx.fillStyle = g; ctx.beginPath(); ctx.moveTo(-5, 8); ctx.lineTo(0, 18+Math.random()*6); ctx.lineTo(5, 8); ctx.closePath(); ctx.fill();
  ctx.shadowBlur = 12; ctx.shadowColor = '#ff6a00'; ctx.fill(); ctx.shadowBlur = 0;
}
// render.js background — from fralonra/star-time-lapse
const stars = Array.from({length: 120}, () => ({x: Math.random()*960, y: Math.random()*600, z: 0.3+Math.random()*1.2, a: Math.random()*Math.PI*2}));
function drawStarfield(ctx, t) {
  ctx.save();
  for (const s of stars) {
    s.a += 0.02 * s.z; const tw = 0.6 + 0.4*Math.sin(s.a), y = (s.y + t*8*s.z) % 600;
    ctx.globalAlpha = 0.35 * tw * (0.6 + 0.4*s.z); ctx.fillStyle = s.z > 0.9 ? '#e8ecff' : '#9aa8c7';
    ctx.fillRect(s.x, y, s.z > 0.8 ? 1.6 : 1, s.z > 0.8 ? 1.6 : 1);
  }
  ctx.restore();
}
```
*Source patterns:* flame gradient is vanilla canvas (inspired by [KAPLAY particle presets](https://github.com/kaplayjs/kaplay) simplified); starfield is literal simplification of [fralonra/star-time-lapse/star.js](https://github.com/fralonra/star-time-lapse/blob/master/star.js) + [fralonra/star-time-lapse demo](https://fralonra.github.io/star-time-lapse/demo/). Add `ctx.globalCompositeOperation='lighter'` for trails and `filter: blur(0.4px)` via CSS for CRT softness.

### When to reach for a LIB (variants)

- **`?variant=proton`** — import Proton for multi-emitter explosions (shock ring + smoke) when 12-line loop isn't enough. `import * as Proton from 'https://cdn.jsdelivr.net/npm/proton-engine/build/proton.min.js'` — swap `spawnExplosion` for `emitter.p.x = x` Proton style.
- **`?variant=rough`** — import Rough.js only around asteroid drawing: `const rc = rough.canvas(canvas); rc.polygon(pts, {roughness: 1.8, stroke:'#cfd6e4'})`. Gives sketched look in 1 line.
- **`?variant=kontra`** — use Kontra's `Pool` for bullet/particle pooling if GC pauses appear at 10k× speed; otherwise don't.

All variants stay GH Pages compatible: one `importmap` entry, no `npm run build`, jsDelivr/cdnjs/unpkg all send `*` CORS and allow anon fan-out per their ToS.

---

## Sources — primary citations per checklist cell

- Awesome lists: [Calinou/awesome-gamedev LICENSE CC-BY-SA](https://github.com/calinou/awesome-gamedev/blob/master/LICENSE.md), [skywind3000/awesome-gamedev LICENSE GPLv3](https://github.com/skywind3000/awesome-gamedev/blob/master/LICENSE.md), [raphamorim/awesome-canvas LICENSE MIT](https://github.com/raphamorim/awesome-canvas/blob/master/LICENSE.md), [brandonhimpfen/awesome-html5](https://github.com/brandonhimpfen/awesome-html5), [proyecto26/awesome-jsgames LICENSE CC0](https://github.com/proyecto26/awesome-jsgames/blob/master/LICENSE), [Siilwyn/awesome-pixel-art license CC0](https://github.com/Siilwyn/awesome-pixel-art/blob/master/license.md).
- Kontra: [_repo](https://github.com/straker/kontra) MIT, CDN `https://cdn.jsdelivr.net/npm/kontra@10/kontra.min.js`, CORS via [jsDelivr](https://www.jsdelivr.com/docs).
- LittleJS: [repo](https://github.com/KilledByAPixel/LittleJS) MIT, dist `https://github.com/KilledByAPixel/LittleJS/tree/main/dist`.
- KAPLAY: [repo](https://github.com/kaplayjs/kaplay) MIT, CDN `https://cdn.jsdelivr.net/npm/kaplay@3000/dist/kaplay.mjs` via [jsDelivr](https://www.jsdelivr.com/docs) + unpkg.
- Phaser: [repo](https://github.com/photonstorm/phaser) MIT, CDN `https://cdn.jsdelivr.net/npm/phaser@3.80.1/dist/phaser.min.js`.
- PixiJS: [repo](https://github.com/pixijs/pixijs) MIT, CDN `https://cdn.jsdelivr.net/npm/pixi.js@7/dist/pixi.min.js`.
- Proton: [repo](https://github.com/drawcall/Proton) MIT, CDN `proton-engine` on jsDelivr.
- Rough.js: [repo](https://github.com/rough-stuff/rough) MIT, `https://cdn.jsdelivr.net/npm/roughjs@4/bundled/rough.js`.
- Patterns: [dmcinnes/HTML5-Asteroids LICENSE MIT](https://github.com/dmcinnes/HTML5-Asteroids/blob/master/LICENSE) + [game.js](https://github.com/dmcinnes/HTML5-Asteroids/blob/master/game.js); [fralonra/star-time-lapse](https://github.com/fralonra/star-time-lapse) + [demo](https://fralonra.github.io/star-time-lapse/demo/) + [star.js](https://github.com/fralonra/star-time-lapse/blob/master/star.js); [CanvasCycle demo](https://www.effectgames.com/demos/canvascycle/) + [awesome-canvas entry](https://github.com/raphamorim/awesome-canvas#examples).
- CDN ToS/CORS: [jsDelivr Terms](https://www.jsdelivr.com/terms), [cdnjs About](https://cdnjs.com/about), [unpkg](https://unpkg.com/), [jsDelivr Docs CORS](https://www.jsdelivr.com/docs).
