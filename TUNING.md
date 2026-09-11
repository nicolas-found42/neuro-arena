# Tuning log

Every constant in `sim/src/config.rs` has a value and a reason. This file is
where the reason lives, next to the evidence for it.

The tool for every entry here is the headless binary, which is a binary target
of the `sim` crate — it compiles against exactly the `sim` API the app uses, so
a measurement made here means something about what you watch in the window.

```sh
cargo run --release --bin neuroarena-headless -- --seed 2026 --generations 40
cargo run --release --bin neuroarena-headless -- --help
```

What it prints, per Generation: shaped Fitness (best and mean), then the raw
Competence Gate numbers (best alive time, Wave reached, Asteroids destroyed,
median alive time), the Species count, the compatibility threshold, and the
Gate's stagnation counter.

## Rules for an entry

1. **Name the seed and the Generation count.** A constant that looks better at
   Generation 10 and worse at Generation 40 has not been tuned, it has been
   overfitted.
2. **Quote the curve, not a single number.** `best` alone is the luckiest member
   of a Population of 100; `mean` is what the Population actually learned.
3. **Change one constant per run.** Two changes and you cannot attribute the
   difference.
4. **Record the rate.** The last line reports measured steps/s. If a change
   makes the run three times slower, that is part of the result.

## Determinism

A run is a function of its seed alone. The worker count changes how long a run
takes and nothing else, so two machines can compare numbers directly:

```sh
neuroarena-headless --seed 2026 --generations 3 --population 40 --workers 1  | grep -v '^#'
neuroarena-headless --seed 2026 --generations 3 --population 40 --workers 8  | grep -v '^#'
# identical, byte for byte
```

That is asserted, not assumed: the `sim` test suite runs the same seed at one
worker and at eight and requires the Fitness curve and the whole Population to
match, and the headless binary is the end-to-end version of the same check.

The browser rendition also leaned on `Map` insertion order: entries iterated in
the order they happened to be inserted, and single-threaded evaluation made
that ordering implicit. The port makes every ordered traversal an explicit
contract instead — Population members evaluate in member order, the novelty
archive appends in member order at a Generation boundary, and asteroid and
bullet removals preserve list order — because pairwise collisions and
nearest-rock sensor ties read those lists in order.

## The port (2026-09-11)

**No constant was retuned.** Every value in `sim/src/config.rs` moved across
from the browser configuration table unchanged; the port's job was to preserve
the World rules, the sensor set, the Network shape and the evolution, and any
change here would have made the port unverifiable. The values that steer
learning therefore still carry the evidence recorded in their research
citations (arXiv:2311.02283 movement reward, arXiv:1006.4959 and arXiv:2608.12534
action entropy, arXiv:1902.03142 behavior-descriptor novelty, arXiv:2209.03618
decaying exploration), which is why those citations are in the source next to
the numbers rather than in a document.

Baseline, for the entries below to be compared against:

```
$ neuroarena-headless --seed 2026 --generations 10
# NeuroArena headless — seed 2026 · 10 Generations · population 100 · 10 workers
# gen        best        mean   aliveT  wave  asteroids    medAT  spec  delta    gate
    1      2885.3       669.5     60.0     0       2300     18.1     1   2.85    0/15
    2      3231.0       787.4     60.0     0       2300     16.2     1   2.70    1/15
    3      3215.5      1135.9     60.0     0       2500     18.3     1   2.55    0/15
    4      3198.2      1055.2     60.0     0       2400     15.5     1   2.40    1/15
    5      3121.3      1152.0     60.0     0       2350     20.9     1   2.25    0/15
    6      4393.1      1149.1     74.4     1       3460     19.0     1   2.10    0/15
    7      3084.2      1266.4     60.0     0       2300     22.4     1   1.95    0/15
    8      4821.4      1383.8     84.0     1       3870     23.8     1   1.80    0/15
    9      3280.2      1148.1     60.0     0       2500     19.5     1   1.65    1/15
   10      3268.4      1251.6     60.0     0       2500     17.8     1   1.50    2/15
# 10 Generations, 1000 Episodes, 1469392 steps (24489.9 sim-seconds) in 0.11s — measured 13393648 steps/s, 223227.5x realtime
# best Genome: fitness 4821.4, Generation 8, alive 84.0s, Wave 1, 3870 asteroids, 26 nodes / 105 connections
```

Correction, same day: review against the spec found breeding consuming one
stream carried across Generations instead of deriving `(run_seed, generation)`
per Generation as ADR 0005 specifies. Fixed — which shifted every Fitness
number, so the baseline above is post-correction. No constant changed, and
one worker versus eight still produces byte-identical output.

Reading it: the mean roughly doubles over ten Generations (669.5 → 1251.6),
the first Wave falls in Generation 6 (74.4 s alive), and the best Genome
(Generation 8) survives 84.0 s and destroys 3870 Asteroids. The Competence
Gate's stagnation counter ticking 1/15 and resetting (Generations 2→3, 9→10)
is the Gate working: it only trips when the median alive time stops keeping
pace with the best.

## Species: one, by measurement (2026-09-11, #5)

**Decision: no constant changed — one Species is the correct outcome on this
landscape.** The open question asked whether `spec` stuck at 1 was a threshold
bug or the right answer; measured from the baseline seed, it is the right
answer, and no defensible threshold change buys anything better.

The mechanism: speciation compares each Genome against its Species
representative with `Genome::distance`, and the dynamic threshold falls from
`DELTA_TARGET_INIT` (3.0) by `DELTA_STEP` (0.15) per Generation until it pins
at the `DELTA_MIN` floor (1.0) from Generation 14. No Genome ever gets even
two thirds of the way to that floor. A throwaway probe bin (deleted; ~60 lines
against the public `Run` API) ran the baseline Population and measured, each
Generation before breeding, every Genome's distance to its nearest Species
representative — exactly the comparison speciation makes — decomposed into the
structural term `(excess+disjoint)/size` and the weight term
`DISTANCE_C3 · mean|Δw|`. Worst case per 50-Generation block, seed 2026:

```
gen   1-500  maxNearest 0.516  maxStructural 0.073  maxWeight 0.511
gen 151-500  maxNearest 0.601  maxStructural 0.139  maxWeight 0.488
gen 301-500  maxNearest 0.633  maxStructural 0.120  maxWeight 0.538
gen 451-500  maxNearest 0.581  maxStructural 0.115  maxWeight 0.500
```

The weight term saturates near 0.5 (weights clamp to ±4 and selection
compresses matching weights), the structural term never exceeds 0.141 in 500
Generations, and topology barely moves: mean connections sit at 105.x at
Generation 40 against the founding 105, maxing at 165 by Generation 500 —
which the division by genome size shrinks to ≤ 0.14 of distance. The 40- and
500-Generation baselines keep `spec` at 1 throughout (delta pinned at 1.00
from Generation 14) while mean Fitness still climbs (669.5 → 2309.4 by
Generation 40; block average ~3119 by Generations 451–500), so the single
Species is not a stalled Population.

### Could any single constant change it?

One constant per run, seed 2026, against the baseline entry above:

| run | spec | mean @40 | best (run) | meanAvg 451–500 | best @500 | rate |
|---|---|---|---|---|---|---|
| baseline (`DELTA_MIN` 1.0) | 1 | 2309.4 | 12273.3 | 3119 | 18706.9 | 11.9M steps/s |
| `DELTA_MIN` 0.4 · 40 Generations | 2 (gen 21) | 1540.6 | 8132.9 | — | — | 13.0M steps/s |
| `DELTA_MIN` 0.3 · 40 Generations | 3 (gen 19) | 1919.1 | 9946.2 | — | — | 13.1M steps/s |
| `DELTA_MIN` 0.4 · 500 Generations | 2–3 | — | — | 2032 | 14392.9 | 12.3M steps/s |

A floor inside the weight spread (pairwise distances span roughly 0.25 mean to
0.66 max) does found 2–3 Species, but:

- **They are weight-noise clusters, not structural niches.** With a 0.3–0.4
  threshold and a structural term that never exceeds 0.14, membership is
  decided almost entirely by mean weight difference. The controller's target
  of 8–12 Species (`SPECIES_COUNT_MIN`/`MAX`) is never approached — the
  threshold just pins at the new floor, saturated exactly as before.
- **They cost learning at both horizons.** Mean Fitness lands 17–33% below
  baseline at Generation 40; over 500 Generations the block averages stay
  300–1000 lower (2032 vs 3119 in the last block) and the best Genome is worse
  too (14392.9 vs 18706.9).
- **Structure-only speciation is unreachable by any threshold.** It would need
  the weight term removed (`DISTANCE_C3 → 0`) *and* a floor below every
  measured structural maximum (≤ 0.14) — two constants against the one-change
  rule — and at that floor the Population, whose topologies are near-identical,
  collapses back into one Species anyway. There is no threshold at which
  "Species" means structure on this landscape.

The landscape is the deeper reason: nothing in the Fitness pays for
topological novelty, so there is no divergence for Species to protect. The
best Genome of the 500-Generation baseline still has the founding shape at
heart — 35 nodes / 130 connections against the founding 26 / 105. The dynamic
threshold is therefore documented as inert in both regimes (pinned at whatever
floor it is given) rather than retuned blindly. No constant changed, so the
determinism contract is untouched; the 1-vs-8-worker check above still passes
byte for byte.

## Open questions

These are observations, not decisions. Each needs its own entry before a
constant moves.

- **Species count is flat at 1.** Answered by measurement (2026-09-11, #5):
  see "Species: one, by measurement" above. The threshold floor sits ~2–3×
  above every distance the Population can reach, splits inside the spread are
  weight-noise clusters that cost Fitness at both horizons, and no threshold
  can make "Species" mean structure here. One Species is the documented
  outcome, not a bug.
- **Alive time sits on the 60-second Wave clock.** A Ship that survives the Wave
  is not scored for surviving the next one until the field is cleared, so
  `waveTimeLimit` and `waveGrowth` are the two constants that decide how much of
  the curve is "survives" rather than "clears".
- **Nothing here has been re-measured against the browser rendition.** The
  browser's numbers are gone with the old save format and were never meant to be
  reproduced bit for bit (ADR 0005).
