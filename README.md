# NeuroArena

Neural networks learn to play Asteroids — evolved live on your Mac, at whatever
speed the machine can manage, with no browser and no build step beyond `cargo`.

This is **Neuroevolution of Augmenting Topologies** (NEAT): each Ship is driven
by a small neural network, and the networks themselves evolve — weights mutate,
nodes and connections are added, and Genomes are crossed over. A shared
Innovation Tracker plus a compatibility-threshold speciation loop let new
structure survive long enough to find a use instead of being crowded out by the
current best.

Each Ship senses its world through 9 Sensor Rays (the Arena is toroidal, so
rocks are seen across the seams), its own velocity, and threat telemetry:
bearing, closeness, closing and lateral velocity of the nearest rock, the
second-nearest rock, threat size, and encirclement pressure. A memory output
feeds back as an input next step, giving the network a learnable internal state.
Outputs: turn left, turn right, thrust, fire — plus that memory channel.

Fitness rewards staying alive, keeping moving, using all controls (action
entropy), and being behaviorally novel early on — the novelty bonus decays over
Generations so exploration gives way to exploitation. The HUD tracks a separate
Competence Gate (alive time, Wave reached, Asteroids destroyed) so you can watch
raw skill apart from the shaped score.

## Running it

```sh
cargo run --release            # the app
cargo run --release --bin neuroarena-headless -- --seed 2026 --generations 40
cargo test                     # the simulation, the headless binary, the renderer
```

macOS only, no `.app` bundle, no signing: a release binary and a window.

For formatting, linting, tests, coverage, dependency checks, and Git hooks, see
[Rust development tooling](docs/development.md).

## Controls

| Control                  | Effect                                                                                        |
| ------------------------ | --------------------------------------------------------------------------------------------- |
| Pause / Resume (`space`) | freeze the run and study what the current Ship is doing                                       |
| Speed slider             | 1× real time up to ∞ "as fast as the machine allows"; the achieved rate is measured and shown |
| Motion (`m`)             | toggle trails and collision echoes; reduced motion preserves all telemetry                    |
| Rays (`r`)               | overlay the Ship's 9 Sensor Rays                                                              |
| Restart                  | replay the pinned seed from Generation 1, or the watched Genome                               |
| New seed                 | roll a fresh seed and start a new lineage                                                     |
| Save (`s`)               | write the best Genome of this run to `saves/seed<N>-g<M>.json`                                |
| Load (`l`)               | load the next save and replay it — breeding switched off                                      |
| Evolve (`e`)             | stop watching and evolve again, starting from the loaded Genome                               |
| Seed field               | click it, type a number, press Enter to run that seed                                         |

The Arena keeps its 960×600 logical shape at any window size, so a seed means
the same thing in a small window and a full-screen one (ADR 0006). Windows can
be resized freely; the sidebar keeps its size so the numbers stay legible.

## What you are looking at

**The Agent wears its mind.** Around the hull sit two rings, both read straight
out of the World:

- the **Perception Corona** — nine arcs, one per Sensor Ray. Each rests out at
  3.4 hull radii and is pulled in toward the Ship as that ray's reading closes,
  so the ring dents inward where the Agent is under pressure, running from cyan
  to amber as it does;
- the **Intent Ring** — four gauges outside it, each where its request acts:
  fire at the nose, thrust at the tail, the turns to port and starboard. The
  fill is the network's raw output mapped onto the arc and the tick is
  `ACTION_THRESHOLD`, the value the simulation actually compares against, so a
  live request reads without relying on it being lit.

Neither is smoothed, extrapolated or invented. An empty corona means the Agent
sensed nothing.

The strip below the Arena is the bench copy of both, with numbers on it, plus
signed threat telemetry. The Network panel colours its inputs by role — Sensor
Rays cyan, threat telemetry amber, the Agent's own channels neutral — and lights
the two strongest incoming weighted signals per node; solid paths are positive,
dashed paths negative. These are weighted activations, not confidence or causal
explanations. A chevron shows actual velocity; the bracket and hull-clearance
label identify the nearest Asteroid geometrically. Collision echoes are bounded
and suppressed above 16×. At small window sizes, the Network becomes a named
output snapshot.

The Arena is a deep field, and the frame it is drawn in holds values past white.
Asteroids are lit minerals under one key light, standing in front of baked
nebulae and a three-layer starfield; everything the simulation _does_ is written
as light and blooms — thruster plumes, bullet tracers, impact flashes, Wave
pulses along the containment seam, and the watched Ship's luminous trail.
Nothing the interface prints can bloom, which is what keeps the panels and the
text sharp without a mask (ADR 0011).

See the [luminous field report](docs/research/luminous-field.md) for this pass's
captures, research chain, and evidence; the
[deep-field observatory report](docs/research/deep-field-observatory.md) and the
[native visual audit](docs/research/native-visual-upgrade.md) record the passes
beneath it.

## Seeds and reproducibility

A run is a function of its seed. The seed is shown in the HUD at all times,
along with the current Generation, so any run is identifiable and shareable as
a pair of numbers.

Episodes evaluate in parallel across the machine's cores, one per core, each
drawing from its own stream derived from `(seed, generation, member index)`.
Breeding stays sequential on its own stream. The result is therefore identical
on any core count, in any scheduling order, on any machine (ADR 0005):

```sh
cargo run --release --bin neuroarena-headless -- --seed 2026 --generations 3 --workers 1
cargo run --release --bin neuroarena-headless -- --seed 2026 --generations 3 --workers 8
```

A saved Genome carries the seed and Generation it came from, so its context is
never lost. The save format is self-describing and versioned; a file whose
`format` or `version` the app does not know is refused with a message naming the
file and the reason, rather than crashing. No compatibility with the retired
browser champion format exists, by decision (ADR 0004).

## How it is built

A Cargo workspace with two members:

- **`sim`** — the Arena, the World and its Episode rules, the Genome and
  Network, the Population and its evolution, and the Run that steps a
  Generation. It has no windowing or GPU dependency; that is a compile-time
  fact rather than a promise, which is what makes the whole simulation testable
  without a display. The headless runner is a binary target of this crate, not a
  separate crate, so it compiles against exactly the API the app uses.
- **`app`** — the window (`winit`), the renderer (`wgpu`), text (`cosmic-text`)
  and the panels. Lyon tessellates the instrument arcs and signed signal paths.
  The frame graph — a high-dynamic-range scene, a bloom chain and a composite
  (ADR 0011) — the 2D geometry and the panel layout are written here; there is
  no UI framework, by decision (ADR 0004).

The vocabulary is the project's own and lives in [`CONTEXT.md`](CONTEXT.md):
Arena, World, Ship, Agent, Asteroid, Wave, Episode, Seam Copy, Sensor Ray;
Genome, Network, Population, Generation, Species, Fitness, Competence Gate,
Innovation Tracker; HUD, Chart. Decisions are recorded in
[`docs/adr/`](docs/adr).

## Tuning

Constants are tuned with the headless binary and every decision is recorded with
its evidence in [`TUNING.md`](TUNING.md), next to the value it justifies.

The fitness shaping follows cited research; the citations sit next to the
constants they justify in [`sim/src/config.rs`](sim/src/config.rs):

- arXiv:2311.02283 — objective decomposition (movement reward)
- arXiv:1006.4959 — sensorimotor entropy bonus
- arXiv:2608.12534 — entropy-augmented fitness
- arXiv:1902.03142 — action-based behavior descriptors (novelty)
- arXiv:2209.03618 — decaying explore/exploit bonus
