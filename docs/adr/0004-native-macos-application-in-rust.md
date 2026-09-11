# ADR 0004 — Native macOS Application in Rust — Browser Rendition Retired

- **Status:** Accepted — supersedes ADR 0001.
- **Date:** 2026-09-11

## Context
The project shipped as a static GH Pages site: vanilla ES modules, nine CDN stylesheets, Rough.js through an importmap, no build step, no CI, no package manifest. Reconnaissance showed the simulation never touched the browser — the World, Sensor Rays, physics, NEAT, evaluation, Population and the runner import no DOM, canvas, `window`, `fetch` or storage API; every browser call lives in the presentation modules. The owner chose to move the whole project to Rust as a native macOS application, as a complete rewrite rather than a transliteration.

## Decision
The project becomes a native macOS application written in Rust, and nothing non-Rust survives. Window and input through `winit`, drawing through `wgpu`, text through `cosmic-text`, threads through `std`. A Cargo workspace keeps a windowing-free simulation crate apart from the app crate, so "the simulation never sees a window" is a compile-time fact.

ADR 0001's chrome lock is void: 98.css, NES.css, hint.css, spinkit, open-props, tabler-icons and monaspace are CSS libraries with no Rust equivalent, and Rough.js's hachure generator has no Rust port. The Arcade Sketch look is therefore not carried over — the application gets a new look (system fonts, plain polygons, macOS-native chrome). The legacy champion JSON format is retired with no compatibility layer; a new self-describing save format replaces it. Distribution is source plus a release binary (`cargo run`), macOS only, no `.app` bundle, no signing.

## Considered Options
- **Rust compiled to wasm in the browser** — rejected: it preserves the HTML/CSS bootstrap and every CDN constraint while delivering none of the native target.
- **Literal transliteration with the JS kept as an oracle** — rejected: it buys syntax, not types; the win only lands if the seams move.
- **Keeping the Arcade Sketch chrome in the native app** — rejected: that look was nine CSS libraries, and hand-drawing Win98 bevels is weeks spent imitating Windows.

## Consequences
One cutover commit deletes `index.html`, `js/**`, `verify.mjs`, `verify-context.mjs`, `dev/*.mjs`, `champions/*.json`, `doc/demo.gif`, `docs/research/**` and `docs/smoke-seed1409546675-2026-08-30.md`; the GH Pages demo goes dark. `README.md` is rewritten for the native application and `TUNING.md` — linked by the README for weeks and never existing — is created as the tuning log. `docs/agents/**`, `LICENSE` and ADR 0003 (fitness shaping) are unchanged. `CONTEXT.md` loses the `Arcade Sketch` term.

The old `verify.mjs` suite survives only as intent: of its 80 checks, about 20 are World rules that stay literally true, about 43 need Rust forms, and 17 assert JavaScript source text or the Rough.js stub echo and disappear.
