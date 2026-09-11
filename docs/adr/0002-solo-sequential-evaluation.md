# ADR 0002 — Solo Sequential Evaluation (One World, One Agent per Episode)

World evaluates one Agent (one Ship + Network) per Episode, driven sequentially by EvolutionRunner over 100 Episodes per Generation, rather than a simultaneous fleet.

Space-Hammer-style determinism and GH Pages simplicity outweigh throughput: one World with one Agent keeps toroidal sensors, elastic Asteroid collisions, and seeded RNG reproducible and headless-verifiable via `node verify.mjs` and `dev/evolve.mjs`, avoids shared-state contention and HUD ambiguity, and lets speed scale to 10 000×. Rejected fleet parallelism (N Worlds or N Agents in one World) would entangle physics, sensing, and rendering and obscure which Genome earned which Fitness.

Consequences: `EvolutionRunner.brainIdx`/`loadBrain` steps through Population order; `World` holds a single `agents[0]`; showcase mode replays one champion Genome in every slot without breeding.

**Superseded by ADR 0005 (2026-09-11):** one World with one Agent per Episode still holds, but Episodes evaluate concurrently, each on a stream derived from `(run_seed, generation, member_index)`. The "GH Pages simplicity" ground for sequential evaluation no longer exists.
