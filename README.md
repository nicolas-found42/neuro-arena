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
| Motion (`m`)             | reduced motion: no trail, no collision echoes, no field tremor; every reading stays           |
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

Where the corona answers "how close is it on bearing k", the **Near Boundary**
answers "where is the wall": a broken line joining the bearings that actually
found something, at the range each one measured, with a dot on every sample.
It is drawn as open chains and never closed into a loop, because the Sensorium
samples nine bearings 40° apart rather than sweeping — a chord across a bearing
that read nothing would be an edge the simulation never measured. With nothing
found on any bearing it draws nothing at all.

The strip below the Arena is the bench copy of both, with numbers on it, plus
signed threat telemetry. The Network panel colours its inputs by role — Sensor
Rays cyan, threat telemetry amber, the Agent's own channels neutral — and draws
the two largest pre-activation terms `Σ w·a` per node as bands whose **opacity**
carries the magnitude, cyan for a positive term and amber for a negative. That
ranking is one hop into the sum the network evaluates, not confidence and not
attribution, and the panel says so. A chevron shows actual velocity; the bracket
and hull-clearance label identify the nearest Asteroid geometrically. When the
topology outgrows the box the panel says how many terms it left out rather than
dropping them quietly. At small window sizes, the Network becomes a named output
snapshot.

**The Record** reads the run at two scales. The lower register is the shaped
Fitness the run actually optimises — best and mean, labelled as the breeding
score rather than as skill (ADR 0003) — with the share of the Population that
cleared the first Wave beside it. The upper register is the Generation itself:
one point per member in (coverage, alive time), tinted by that member's own
novelty, filling in as Episodes land, with the previous Generation kept behind
it as a ghost and the watched member ringed. Mean Waves is not drawn: at shipped
tuning a Generation holds one to three distinct Wave values, so the honest curve
is the score that moves.

The Arena is a deep field with depth: four sky layers — far dust, the mid field,
near stars and the motes closest to the hull — displaced against the watched
Ship's own velocity, so the field reads as a volume the Ship moves through
rather than wallpaper behind it. The same depth decides how much of the field's
tremor each layer takes. The frame holds values past white, so everything the
simulation _does_ is written as light and blooms: thruster plumes, a bullet's
tracer, a muzzle flash, a shock front opened at the Impact's own recorded radius
with its spill on the rock, Wave pulses along the containment seam, and the
watched Ship's tapered ribbon of a trail, stored unwrapped so a wrap leaves one
edge and arrives at the other. The event language runs at every speed — the
cosmetic layer is handed the last quarter-second of simulation before each
frame rather than being switched off above ×16 — so a default run at ×100 shows
it. Nothing the interface prints can bloom, which is what keeps the panels and
the text sharp without a mask (ADR 0011).

See [The field and the record](docs/research/field-and-record.md) for this pass's
captures, sources and measurements, the
[luminous field report](docs/research/luminous-field.md) for the HDR pass, and the
[observatory, legible report](docs/research/observatory-next.md) for the pass that gave
the bench its minimum-window legs, the frame its idle light and the routing that put
the Ship's instruments on the Ship; the
[deep-field observatory report](docs/research/deep-field-observatory.md) and
the [native visual audit](docs/research/native-visual-upgrade.md) record the passes
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
Innovation Tracker; HUD, Record, Cohort, Near Boundary. Decisions are recorded in
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
