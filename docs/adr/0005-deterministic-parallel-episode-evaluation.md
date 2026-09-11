# ADR 0005 — Deterministic Parallel Episode Evaluation — Streams Derived From (Run Seed, Generation, Member Index)

- **Status:** Accepted — supersedes ADR 0002.
- **Date:** 2026-09-11

## Context
ADR 0002 chose one World with one Agent per Episode, evaluated sequentially, and justified that by "GH Pages simplicity" and headless verifiability. Both grounds disappear in a native application, where a Population can be spread across cores; parallel Episode evaluation is the largest throughput win available. The simulation already anticipated this: `js/rng.js` exposed `createRNG(seed)` so independent streams could be threaded through World, Population and Genome instead of one global stream.

## Decision
A Generation still evaluates each member of the Population in its own World with one Agent, and every World rule, constant and Episode cap is unchanged. Episodes now evaluate concurrently. Each Episode draws from its own stream derived from `(run_seed, generation, member_index)`, so a run yields identical results on 8 cores and on 16. Breeding stays sequential on a stream derived from `(run_seed, generation)`. The seed is a first-class contract: a run is reproducible from a seed plus a Generation.

## Considered Options
- **Keep one global stream, consumed in Population order** — rejected: it serializes evaluation, which is the parallelism this decision exists to buy.
- **Change the rules — a fleet in one Arena, GPU-batched simulation** — rejected: a research project, not a rewrite; it entangles physics, sensing and rendering again, which is the part of ADR 0002 that was right.

## Consequences
Reproducibility no longer depends on thread count or scheduling, and a headless binary can replay a run and print its Fitness curve as the tuning oracle. There is no number compatibility with the browser rendition: with the champion format retired (ADR 0004), old runs are not reproducible and the old `?seed=` share links are gone. The 60-second Wave clock, the 300-second Episode cap and the fixed timestep persist.
