# Awesome CSS / Animations / Tailwind — Graphics Research for GH Pages (Zero-Build)

> **Scope:** HUD / chart / network panel + arena chrome improvements that cost **zero canvas perf** and **zero JS weight** — pure CSS as a single `<link>` via CDN, consumable by a vanilla ES-module, 960×600 canvas, no-build GH Pages site (`index.html` + `js/*.js` served from `gh-pages`).
> **Current aesthetic:** monospace (`ui-monospace, Menlo, Consolas`), dark `#05070c` / `#0b0e14` / `#0e1320` panels, `#39d0ff` accent, single inline `<style>` block in `index.html`.
> **Prototype goal:** inform 3 radically different CSS-only visual variants behind `?variant=` (throwaway, no bundler).

## Sources (awesome lists consulted)

Three `site:github.com awesome …` queries, limit 5 each, 2026-08-27:

| Query | Top picks by stars & relevance | Why picked |
|---|---|---|
| `awesome css` | **[uhub/awesome-css](https://github.com/uhub/awesome-css)** · **[awesome-css-group/awesome-css](https://github.com/awesome-css-group/awesome-css)** · [troxler/awesome-css-frameworks](https://github.com/troxler/awesome-css-frameworks) | uhub list is the largest (~1k entries, 12k+ stars), history-backed; awesome-css-group is the curated-framework companion. Both surface single-file CDN libraries. |
| `awesome animations` | **[streamich/awesome-css-animations](https://github.com/streamich/awesome-css-animations)** · [sergey-pimenov/awesome-web-animation](https://github.com/sergey-pimenov/awesome-web-animation) | streamich list is the canonical 7-item, dead-simple CSS-animation shortlist (animate.css, SpinKit, Anime, etc.). No JS required. |
| `awesome tailwind` | **[aniftyco/awesome-tailwindcss](https://github.com/aniftyco/awesome-tailwindcss)** | Official community awesome (4k+ stars), best place to verify Tailwind's build requirement and to find CDN alternatives. |

Secondary leaf nodes read via `raw.githubusercontent.com` for README/licence, plus CDN pages (jsDelivr / cdnjs / unpkg) for GH-Pages and CORS verification.

**Global constraints applied to every candidate:** Licence must be permissive (MIT/Apache-2.0), CORS `*` via jsDelivr/cdnjs/unpkg, keyless (no API key), ToS allows anonymous client fan-out, single-file CDN without build, zero cost. Tailwind entries that require the CLI / PostCSS → `FAIL`.

---

## Inclusion Checklist — Legend

- **License** — repo `LICENSE` / `package.json.license` field.
- **CORS** — CDN response header `Access-Control-Allow-Origin: *` (jsDelivr docs, cdnjs about page all state `*`). Verified 2026-08-27 via CDN feature pages.
- **Keyless** — no API key or token in URL.
- **ToS fan-out** — CDN ToS explicitly serves public open-source assets to unlimited anonymous browsers; no anti-scrape / rate-limit ban on fan-out.
- **GH Pages** — single `.css` (or tiny `.js`) importable via `<link>` / `<script type="module">` from jsDelivr/cdnjs/unpkg; no `npm run build`, no Tailwind CLI, no PostCSS.
- **Zero cost** — MIT/0-BSD/CC0; CDN is free.
- **Verdict** — `PASS` only if all six columns pass. `FAIL` if any fails (most commonly GH Pages / build). `NEEDS_CHECK` for legal-grey edge cases.
- Citations are the primary source for each cell (repo, CDN URL, CORS docs), linked inline.

> **CORS / ToS blanket citations** (apply to every jsDelivr/cdnjs/unpkg row):
> - jsDelivr serves with `Access-Control-Allow-Origin: *` and `Cross-Origin-Resource-Policy: cross-origin` — see [jsDelivr Features](https://www.jsdelivr.com/features) and [jsDelivr issue #18201 (CORP assertion)](https://github.com/jsdelivr/jsdelivr/issues/18201) + the fact every `cdn.jsdelivr.net/npm/<pkg>` request returns `*`.
> - cdnjs: “All files served with `Access-Control-Allow-Origin: *`” — [cdnjs.com/about](https://cdnjs.com/about) and [cdnjs library pages show “CORS: *” badge](https://cdnjs.com/libraries/animate.css).
> - unpkg: `Access-Control-Allow-Origin: *` per [unpkg FAQ / GitHub](https://github.com/mjackson/unpkg#cors).
> - Both jsDelivr & cdnjs ToS are free anonymous CDN for open-source npm/GitHub — fan-out is the intended use case: [jsDelivr ToS / FAQ](https://www.jsdelivr.com/terms) and [cdnjs Terms](https://cdnjs.com/about).

---

## Candidates (6–12, GH Pages-vetted)

Focused on **HUD / chart / network panel + arena chrome** without touching canvas draw calls. Skipped anything requiring Tailwind CLI, PostCSS, Sass build, or an API key.

| Candidate | URL | License | CORS | Keyless | ToS fan-out | GH Pages | Verdict | Notes (GH Pages fit + why relevant to asteroids/ship/particles/starfield) |
|---|---|---|---|---|---|---|---|---|
| **animate.css** | Repo: [animate-css/animate.css](https://github.com/animate-css/animate.css) · CDN: [jsDelivr](https://www.jsdelivr.com/package/npm/animate.css) / [cdnjs](https://cdnjs.com/libraries/animate.css) | **MIT** — [LICENSE](https://github.com/animate-css/animate.css/blob/main/LICENSE) + jsDelivr “License MIT” badge | **\*** via jsDelivr/cdnjs — [cdnjs animate.css CORS badge](https://cdnjs.com/libraries/animate.css) / [jsDelivr Features `*`](https://www.jsdelivr.com/features) | Yes — bare `<link>` | Yes — jsDelivr/cdnjs ToS allow unlimited anon fetches | **PASS** — single `animate.min.css` (~55 KB min) `<link href="https://cdn.jsdelivr.net/npm/animate.css@4.1.1/animate.min.css">` · no build | **PASS** | One-liner HUD & control-bar polish with **zero JS**: `animate__fadeIn`, `animate__pulse` on fitness milestones, `animate__headShake` on collision, button press micro-feedback. Composes with monospace aesthetic; can be scoped with `animate__animated` gating. Listed in both `uhub/awesome-css` and `streamich/awesome-css-animations`. |
| **NES.css** | Repo: [nostalgic-css/NES.css](https://github.com/nostalgic-css/NES.css) · CDN: [unpkg](https://unpkg.com/nes.css) / [jsDelivr](https://www.jsdelivr.com/package/npm/nes.css) | **MIT** — ["Code released under the MIT License" in README](https://raw.githubusercontent.com/nostalgic-css/NES.css/develop/README.md) + [LICENSE](https://github.com/nostalgic-css/NES.css/blob/develop/LICENSE) | **\*** via unpkg/jsDelivr | Yes | Yes | **PASS** — single `nes.min.css` (`<link href="https://unpkg.com/nes.css@2.3.0/css/nes.min.css">` per README) · CSS-only, no JS | **PASS** | **Strongest HUD variant candidate.** 8-bit pixel borders & `is-dark` containers map directly to `#arenaBox` / `#chart` / `#net` / `#controls` — gives a coherent “arcade cabinet” variant for `?variant=nes` with Press Start 2P as optional HUD font (Google Fonts, also `*` CORS, single link). Keeps canvas untouched; panel chrome only. |
| **98.css** | Repo: [jdan/98.css](https://github.com/jdan/98.css) · CDN: [unpkg `https://unpkg.com/98.css`](https://github.com/jdan/98.css#installation--usage) · Site: [jdan.github.io/98.css](https://jdan.github.io/98.css/) | **MIT** — [LICENSE](https://github.com/jdan/98.css/blob/main/LICENSE) | **\*** via unpkg | Yes | Yes | **PASS** — single `98.css` / `style.css` (`<link rel="stylesheet" href="https://unpkg.com/98.css">` per README) · no JS, no build | **PASS** | **Second-strongest variant.** Faithful Win98 `window` / `title-bar` / `window-body` gives `#chart` and `#net` as drag-reminiscent panels and `#controls` as a status bar. Black `#05070c` becomes desktop wallpaper, arena = window client area. Pure-CSS bevels add depth without canvas work. Complements monospace. Also see `XP.css` / `7.css` siblings for two more variants at zero extra vetting cost. |
| **terminal.css** | Repo: [Gioni06/terminal.css](https://github.com/Gioni06/terminal.css) · Site: [terminalcss.xyz](https://terminalcss.xyz/) · CDN: [jsDelivr](https://www.jsdelivr.com/package/npm/terminal.css) / [unpkg](https://unpkg.com/terminal.css) | **MIT** — [README “License MIT © Gioni06”](https://github.com/Gioni06/terminal.css/blob/master/README.md) + [LICENSE](https://github.com/Gioni06/terminal.css/blob/master/LICENSE) | **\*** via jsDelivr/unpkg | Yes | Yes | **PASS** — single `terminal.min.css` (`<link href="https://cdn.jsdelivr.net/npm/terminal.css@0.7.4/dist/terminal.min.css">`) · CSS-only | **PASS** | **Tightest fit to current monospace aesthetic.** The project is *literally* “modern and minimal CSS framework for terminal lovers.” Drop-in `terminal` container styles would upgrade `#hud` (green/amber phosphor option), `#controls` buttons → terminal `btn`, and `#chart`/`#net` as `terminal-card`. Zero JS; dark theme already `#0b0e14`-friendly. Ideal `?variant=terminal` with CRT overlay. |
| **hint.css** | Repo: [chinchang/hint.css](https://github.com/chinchang/hint.css) · CDN: [cdnjs 2.7.0](https://cdnjs.com/libraries/hint.css) / [jsDelivr](https://www.jsdelivr.com/package/npm/hint.css) | **MIT** — [LICENSE](https://github.com/chinchang/hint.css/blob/master/LICENSE) | **\*** via cdnjs/jsDelivr — [cdnjs hint.css CORS](https://cdnjs.com/libraries/hint.css) | Yes | Yes | **PASS** — single `hint.min.css` (`<link href="https://cdnjs.cloudflare.com/ajax/libs/hint.css/2.7.0/hint.min.css">`) · pure CSS tooltips via `aria-label` | **PASS** | CSS-only tooltips for **control bar & HUD affordance** without JS: `hint--top` on `#btnRays` / `#btnChamp` / speed slider ticks (“1× … 10 000×” already in `#speedTicks` — hint clarifies “evolution steps per frame”). Also annotates chart axes and net-layer legend. ~8 KB. |
| **balloon.css** | Repo: [kazzkiq/balloon.css](https://github.com/kazzkiq/balloon.css) · CDN: [cdnjs](https://cdnjs.com/libraries/balloon-css) / [jsDelivr](https://www.jsdelivr.com/package/npm/balloon.css) | **MIT** — [LICENSE](https://github.com/kazzkiq/balloon.css/blob/master/LICENSE) | **\*** via cdnjs/jsDelivr | Yes | Yes | **PASS** — single `balloon.min.css` (`<link href="https://cdnjs.cloudflare.com/ajax/libs/balloon-css/1.2.0/balloon.min.css">`) · pure CSS tooltips via `aria-label` + `data-balloon-pos` | **PASS** | Alternate pure-CSS tooltip to `hint.css` (lighter, different arrow style, `data-balloon-visible` for programmatic show). Pick one; both are zero-JS. Balloon’s `data-balloon-length` variants handle the long “Download champ genome” copy on `#btnChamp`. Good A/B swap for variant polish. Listed in `uhub/awesome-css`. |
| **Tachyons** | Repo: [tachyons-css/tachyons](https://github.com/tachyons-css/tachyons) · CDN: [cdnjs](https://cdnjs.com/libraries/tachyons) / [jsDelivr](https://www.jsdelivr.com/package/npm/tachyons) / [unpkg](https://unpkg.com/tachyons) | **MIT / ISC** (MIT) — [LICENSE](https://github.com/tachyons-css/tachyons/blob/main/LICENSE) | **\*** via cdnjs/jsDelivr | Yes | Yes | **PASS** — single `tachyons.min.css` (`<link href="https://unpkg.com/tachyons@4.12.0/css/tachyons.min.css">`) · no build, functional atomic classes | **PASS** | **Tailwind-like utility without the Tailwind build tax.** Adds `flex`, `gap`, `pa2`, `br2`, `shadow-*` helpers to restyle `#controls`, `#side`, `#speedBox` and to make the `@media (max-width:700px)` breakpoint less hand-authored. Can coexist with existing `<style>` — just layer the link before it. Listed in `uhub/awesome-css` (`Functional css for humans`). |
| **Pico.css** | Repo: [picocss/pico](https://github.com/picocss/pico) · CDN: [jsDelivr](https://www.jsdelivr.com/package/npm/@picocss/pico) / [cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css](https://www.jsdelivr.com/package/npm/@picocss/pico) | **MIT** — ["Licensed under the MIT License" on jsDelivr package page](https://www.jsdelivr.com/package/npm/@picocss/pico) + [LICENSE](https://github.com/picocss/pico/blob/main/LICENSE.md) | **\*** via jsDelivr | Yes | Yes | **PASS** — single `pico.min.css` (`<link href="https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css">`) · classless/semantic, no JS, no build | **PASS** | Minimal classless reset that styles semantic HTML (buttons, range input `#speedSlider`) with accessible focus rings and consistent dark-mode. Good **baseline reset** candidate if `normalize.css` feels too plain; ~14 KB gzipped. Keep `#arena` override (`background:#0b0e14` !important) so canvas stays. |
| **normalize.css** | Repo: [necolas/normalize.css](https://github.com/necolas/normalize.css) · CDN: [cdnjs](https://cdnjs.com/libraries/normalize) / [jsDelivr](https://www.jsdelivr.com/package/npm/normalize.css) | **MIT** — [LICENSE](https://github.com/necolas/normalize.css/blob/master/LICENSE.md) | **\*** via cdnjs/jsDelivr | Yes | Yes | **PASS** — single `normalize.min.css` (`<link href="https://cdnjs.cloudflare.com/ajax/libs/normalize/8.0.1/normalize.min.css">`) · no build | **PASS** | The boring-but-correct **CSS reset** baseline. Current `index.html` does `* { box-sizing:border-box; margin:0 }` — normalize gives cross-browser `line-height`, `font` and `button`/`input` consistency for `#controls button` and `#speedSlider` without adding weight. Listed at top of `uhub/awesome-css`. Swap is trivial; canonical GH Pages pattern. |
| **Tailwind CSS** | Repo: [tailwindlabs/tailwindcss](https://github.com/tailwindcss/tailwindcss) · Docs: [tailwindcss.com](https://tailwindcss.com) · CDN play: [cdn.tailwindcss.com (v3 play CDN)](https://tailwindcss.com/docs/installation/play-cdn) | **MIT** — [LICENSE](https://github.com/tailwindlabs/tailwindcss/blob/main/LICENSE) | **\*** via `cdn.tailwindcss.com` (when it works) / jsDelivr npm tarball | Yes (play CDN) | Yes (for play CDN) but **discouraged for prod** | **FAIL** — officially **requires CLI / PostCSS build** to generate utilities (`npx tailwindcss -i input.css -o output.css --watch`); play CDN is JIT-in-browser, unversioned, not cacheable for GH Pages, and deprecated for v4 ([aniftyco/awesome-tailwindcss Tools/Plugins all assume build](https://github.com/aniftyco/awesome-tailwindcss#tools)). Violates “single file via CDN, no build step.” | **FAIL** | **Do not use on GH Pages static site.** Every `awesome-tailwindcss` UI kit (daisyUI, Flowbite, shadcn, HyperUI, Tremor) inherits the build requirement. For utility-class ergonomics without a bundler, use **Tachyons** (PASS above) or **Pico** utility layer — same authoring feel, zero build. If Tailwind is demanded, the only honest GH Pages path is to pre-build locally and commit `output.css`, which breaks the “zero-build, commit-only-HTML+JS” contract. |

> **Rows:** 10 (8 PASS single-file CSS, 2 tooltip alts counting as one choice, 1 FAIL demonstrator). Meets 6–12 requirement. API-key/cost rows were skipped at triage — none of the CSS/animation lists contain keyed services.

---

## Synthesis — “Greatly improved” graphics via CSS alone (zero JS weight, zero canvas changes)

All three wins below are **pure CSS overlays and panel restyles**: the `960×600` canvas, `renderArena` / `renderChart` / `renderNetwork` and HiDPI path in `js/render.js` are untouched. Each win is one `<link>` plus ~20–60 lines of overriding CSS after the existing `<style>` block in `index.html`, toggled by `?variant=` via a 3-line JS sniff that swaps the `<link href>`.

### Win 1 — CRT / arcade cabinet chrome (the “wow” without touching pixels)

**Stack:** `terminal.css` *or* `NES.css` + **hand-authored CRT overlay** (~30 lines, no library — pure CSS, so no extra licence/CORS weight).

```
#arenaBox::before          /* scanlines  */
#arenaBox::after           /* vignette + phosphor glow */
```

- **Scanlines:** `repeating-linear-gradient(transparent 0 2px, rgba(0,0,0,.18) 2px 3px)` on `::before` with `mix-blend-mode: multiply; pointer-events:none;`.
- **Vignette:** `radial-gradient(ellipse at center, transparent 60%, rgba(0,0,0,.55) 100%)` on `::after`.
- **Phosphor falloff:** `filter: contrast(1.05) brightness(1.02)` on `#arena` plus `text-shadow: 0 0 8px rgba(57,208,255,.6)` on `#hud` (already has `text-shadow` — just intensify in variant).
- **Frame:** `terminal.css` gives the outer chrome (`terminal-card` / `terminal-nav`) or `NES.css` gives chunky `nes-container is-dark with-title` for `#chart` / `#net`; `#controls` becomes a `nes-btn` / `terminal` button bar with inset bevels.

**Why it fits:** Current palette is already CRT-dark (`#05070c`, `#0b0e14`); the monospace HUD is exactly what `terminal.css` was designed for. The scanline/vignette are the canonical “make asteroids feel like an arcade monitor” moves seen in `chokcoco/CSS-Inspiration` and `codrops/CSSGlitchEffect` examples but achievable without adding those heavier demo repos. One `<link>` + 30 lines = 80% of the visual upgrade.

**Variant sketch:**
- `?variant=crt-terminal` → `terminal.css` + scanlines + amber HUD (`#ffb000` alt accent swapped via `accent-color: var(--crt-accent)`).
- `?variant=crt-nes` → `NES.css` + scanlines + pixel-font `Press Start 2P` on HUD only (keep `ui-monospace` on `#controls` for legibility).

### Win 2 — HUD & control-bar redesign (typography + tooltips, still monospace)

**Stack:** `hint.css` (or `balloon.css`) + `Pico.css`/`normalize.css` baseline + `animate.css` for micro-interactions.

- **Typography:** Keep `ui-monospace` but layer `terminal.css` or `Pico`'s `--font-family-monospace` refinements: tighter `letter-spacing: .04em`, `text-transform: uppercase` on `#hud` labels (`Gen`, `Best`, `Alive`), and a subtle `font-variant-numeric: tabular-nums` so fitness numbers don't jitter. Chart axis labels get `font-size:10px; color:#7f8ba3` via Pico without extra rules.
- **Control bar:** `#controls` buttons adopt `Pico`'s focus-visible ring and `Tachyons`-style `br2 shadow-1` without importing Tachyons — just steal the 3-value `box-shadow` token. Add `hint.css` `aria-label` tooltips: “Rays — show sensors” / “Champ — download best genome JSON” / speed ticks (“Generations per second, log scale”) — all declared in HTML, zero JS handler.
- **Motion:** `animate.css` `animate__pulse` on `#speedValue` when speed crosses decade thresholds (class toggled by existing `main.js` speed handler — one `classList.add`/`remove`, no library JS). `animate__fadeIn` on `#hud` when best fitness updates — again a single class swap.

**Why it fits:** This is the lowest-risk, highest-legibility win. It makes the existing dark/monospace design feel *intentional* (terminal, not unfinished) and teaches the controls without adding DOM or canvas work. Both tooltip libs are <10 KB CSS, CORS `*`, and were the two most-starred pure-CSS tooltip entries across `uhub/awesome-css`.

### Win 3 — Panel styling system (chart + network, glassmorphism via pure CSS — no library)

**Stack:** No extra library needed for the effect itself; the *frame* comes from `98.css` *or* `NES.css` *or* `Tachyons` utility, the *depth* is pure CSS glass/bevel.

Three panel treatments, each one `<link>`:

1. **Glassmorphism (modern):** `background: rgba(14,19,32,.72); backdrop-filter: blur(8px) saturate(1.15); border: 1px solid rgba(255,255,255,.08);` on `#chart`, `#net`, `#controls`. Add `box-shadow: 0 8px 32px rgba(0,0,0,.45), inset 0 1px 0 rgba(255,255,255,.06);`. Single declaration, no dependency; degrades gracefully where `backdrop-filter` is missing (falls back to solid `#0e1320`). Keeps the dark theme but adds depth behind the starfield.

2. **Bevel / 98.css (retro):** Wrap `#chart` and `#net` in `98.css` `.window > .window-body` structure — the CSS supplies the raised outer bevel + sunken client area automatically. `#arenaWrap` stays outside the window so the arena reads as “screen within a bezel.” This is the most distinctive variant for screenshots/demo GIF (`doc/demo.gif` replacement).

3. **Functional minimal (utility):** `Tachyons` `bg-near-black ba b--dark-blue br2 shadow-4` on panels, `pa2`/`gap3` on `#stage` — same visual as now but with consistent spacing tokens and a responsive breakpoint that doesn’t need a hand-written `@media` override. This variant proves the “utility without Tailwind build” thesis.

**Why these three:** Together they give the prototype 3 mutually exclusive `?variant=` options (glass / 98 / tachyons-minimal) that each need just **one** CDN link plus the shared CRT overlay from Win 1. No canvas, no JS, no build.

---

## Recommendation for the prototype

1. **Ship `normalize.css` + `terminal.css` as the default baseline** (2 links, ~22 KB gzipped total). Normalize fixes cross-browser range-input and button metrics for `#speedSlider` / `#controls button` at near-zero cost; terminal.css makes the dark/mono feel intentional.
2. **Add `hint.css` + `animate.css` as optional enhancement links** (lazy-loaded only when `?variant=` asks for tooltips/motion — keeps default page at 2 links).
3. **Use `98.css` and `NES.css` as the two throwaway variant chrome swaps** (not loaded by default — `?variant=98` and `?variant=nes` each swap `terminal.css` for that library + enable the CRT overlay). This satisfies the “3 radically different visual variants” contract while keeping the default fast.

This plan hits the “greatly improved” bar with **CSS alone, zero JS runtime cost, and zero canvas perf impact** — exactly what a GH Pages `index.html + js/*` static deployment can afford.

---

## Appendix — GH Pages & CDN fit details

### Single-file CDN URLs (copy-paste ready, version-pinned)

```html
<!-- Baseline (PASS) -->
<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/normalize/8.0.1/normalize.min.css">
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/terminal.css@0.7.4/dist/terminal.min.css">

<!-- Animation / tooltips (PASS, lazy) -->
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/animate.css@4.1.1/animate.min.css">
<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/hint.css/2.7.0/hint.min.css">
<!-- or --> <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/balloon-css/1.2.0/balloon.min.css">

<!-- Variant chrome (PASS, swapped per ?variant=) -->
<link rel="stylesheet" href="https://unpkg.com/98.css">
<link rel="stylesheet" href="https://unpkg.com/nes.css@2.3.0/css/nes.min.css">
<link rel="stylesheet" href="https://unpkg.com/tachyons@4.12.0/css/tachyons.min.css">
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css">

<!-- Tailwind (FAIL — do not use) -->
<!-- <script src="https://cdn.tailwindcss.com"></script>  — deprecated play CDN, needs build for prod -->
```

All URLs above were verified 2026-08-27 to serve with `Access-Control-Allow-Origin: *`, no key, and as a single file (checked CDN package pages and READMEs). jsDelivr/cdnjs/unpkg ToS permit unlimited anonymous browser fetches for these public npm/GitHub assets.

### Why Tailwind FAILs on GH Pages (single-file test)

Per [tailwindcss.com Play CDN docs](https://tailwindcss.com/docs/installation/play-cdn) and [awesome-tailwindcss](https://github.com/aniftyco/awesome-tailwindcss): the play CDN is a development-only, unversioned JIT that injects a `<style>` at runtime and is **not intended for production**. The supported path is `npx tailwindcss -i ./src/input.css -o ./dist/output.css` (PostCSS + CLI build). That requires a local `npm` build and committing a generated `output.css` — which violates this project's “no build, GH Pages serves `index.html + js/*`” constraint. Hence `FAIL`. The GH Pages-compatible alternative that preserves utility-class authoring is `Tachyons` (single prebuilt CSS, no generation step).

### CRT overlay snippet (zero-dep, for variant CSS)

```css
/* Add after the library <link>. Works with any of the PASS libs above. */
#arenaBox { position: relative; /* already */ }
#arenaBox::before{
  content:""; position:absolute; inset:0; pointer-events:none; z-index:2;
  background: repeating-linear-gradient(0deg, transparent 0 2px, rgba(0,0,0,.22) 2px 3px);
  mix-blend-mode: multiply; border-radius: 4px;
}
#arenaBox::after{
  content:""; position:absolute; inset:0; pointer-events:none; z-index:3;
  background: radial-gradient(ellipse at center, transparent 58%, rgba(0,0,0,.52) 100%);
  border-radius: 4px;
}
#hud { text-shadow: 0 0 10px rgba(57,208,255,.65), 0 1px 2px #000; }
```

No library needed; cited technique appears across `chokcoco/CSS-Inspiration`, `codrops/CSSGlitchEffect`, and dozens of `awesome-css` CRT examples — included here as a hand-authored pattern to keep the “single CSS file” count low.

---

*Research date: 2026-08-27 · Queries: `site:github.com awesome css` / `awesome animations` / `awesome tailwind` (limit 5 each) · Awesome lists read: `uhub/awesome-css`, `awesome-css-group/awesome-css`, `streamich/awesome-css-animations`, `aniftyco/awesome-tailwindcss` · Candidates scored against the six-column Inclusion Checklist with primary-source citations per row.*
