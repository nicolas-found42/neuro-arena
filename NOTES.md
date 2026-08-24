# NOTES.md — raw notes on the user's world

> Sharpened from grilling 2026-08-24. Canonical terms are **bolded** on first use.

## User & repo
- **Workspace**: `nicolas-found42/neural-network-game` — Neuroevolution Asteroids (NEAT, browser, zero-deps, ES modules).
- **Audience**: maintainer + public via GitHub Pages demo.
- **Repo conventions**: `AGENTS.md` (caution bias, simplicity first, surgical changes), `verify.mjs` is the contract gate, `js/config.js` is tuning surface, `champions/*.json` are exported genomes, `TUNING.md` is evidence log.

## Tools / channels
- **Terminal**: Ghostty 1.3.1, macOS darwin arm64, `gh` CLI inside clone (inferred from `docs/agents/issue-tracker.md`).
- **Issue tracker**: GitHub Issues for `nicolas-found42/neural-network-game` — `gh issue` / `gh pr` per `docs/agents/issue-tracker.md`. Triage labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`.
- **Verification surface**: `node verify.mjs` (headless contract), `node dev/evolve.mjs --seed N --gens 40` (tuning evidence), probe scripts.
- **Hub / long-running processes**: `hub` (omp) for daemons; this repo has no `workflows/` CI yet — loop will run as a hub process.

## Loops observed (candidate workflows)
- **Tweak → evolve → probe → retune** (touch `js/config.js`, run evolve, probe behavior).
- **Verify → ship to Pages** (check `verify.mjs`, push to `main`, GH Pages).

## Grilled loop: architecture-loop
- **User term**: "run the improve codebase architecture skill, then addresses the top issue, then commits directly to main and repeats"
- **Sharpened**:
  - **Architecture audit** = the `improve-codebase-architecture` skill run (Explore → HTML report with candidate cards + Top recommendation).
  - **Top issue** = the **Top recommendation** card from that HTML report — the skill author's pick — verbatim, no re-ranking. If no actionable candidate exists, iteration is a no-op.
  - **Address** = implement the deepening fully: new module interface at a clean **seam**, migrate every caller (clean cutover, no shims), delete obsolete code, update `CONTEXT.md`/`docs/adr/` lazily in same commit when warranted.
  - **Commit directly to main** = single commit per iteration, `git push` to `origin/main` with no PR and no human checkpoint. User explicitly accepted drift risk after push-right alternative was offered.
  - **Repeat infinitely** = tight `while true` chain inside one long-running process; next iteration starts as soon as the push completes. Termination is manual (user stops the process).
  - **Trigger vocabulary**: manual start (human starts the hub process); subsequent iterations are self-triggered by loop continuation, not by external event or schedule.
  - **Checkpoint**: none — fully autonomous per user direction (push-right with brief was recommended and declined).
  - **Verification gate**: `node verify.mjs` must exit 0 or iteration is discarded (hard reset, no commit).

## Terminology (canonical)
- **Loop** / **Workflow** / **Trigger** / **Checkpoint** / **Brief** / **Push right** — per loop-me vocabulary.
- **Module / Interface / Seam / Adapter / Depth / Leverage / Locality / Deletion test** — per `skill://codebase-design`.
- **Candidate / Top recommendation / Recommendation strength (Strong/Worth exploring/Speculative)** — per `skill://improve-codebase-architecture` report.
