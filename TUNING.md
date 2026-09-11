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
# gen        best        mean   aliveT  wave     rocks    medAT  spec  delta    gate
    1      2885.3       669.5     60.0     0      2300     18.1     1   2.85    0/15
    2      3300.2      1007.4     60.0     0      2500     23.3     1   2.70    0/15
    3      3278.5      1057.8     60.0     0      2500     20.7     1   2.55    0/15
    4      3341.8      1102.6     60.0     0      2500     21.2     1   2.40    0/15
    5      3210.6      1162.6     60.0     0      2400     22.3     1   2.25    0/15
    6      3247.0      1125.8     60.0     0      2400     15.7     1   2.10    1/15
    7      3247.4      1121.6     60.0     0      2400     16.4     1   1.95    2/15
    8      4686.6      1228.6     77.1     1      3700     18.7     1   1.80    0/15
    9      3329.8      1168.0     60.6     1      2600     17.2     1   1.65    1/15
   10      3884.1      1356.1     65.0     1      2980     18.5     1   1.50    2/15
# 10 Generations, 1000 Episodes, 1511361 steps (25189.3 sim-seconds) in 0.13s
#   — measured 12 059 561 steps/s, ≈201 000× real time, 10 workers, Apple M5
# best Genome: fitness 4686.6, Generation 8, alive 77.1s, Wave 1, 3700 rocks
```

Reading it: the mean roughly doubles over ten Generations, and by Generation 8
some member clears the first Wave — the Population is learning to survive the
Wave clock rather than to outlive it. The Competence Gate's stagnation counter
ticking 1/15, 2/15 and resetting is the Gate working: it only trips when the
median alive time stops keeping pace with the best.

## Open questions

These are observations, not decisions. Each needs its own entry before a
constant moves.

- **Species count is flat at 1 through Generation 10.** Compatibility distance
  is dominated by the matching-weight term (`distanceC3 = 0.4` × a mean weight
  difference of about 0.7 ≈ 0.28) while `deltaTarget` only falls from 3.0 toward
  the `deltaMin` floor of 1.0, so a Population that starts with one topology
  stays one Species until enough excess genes exist to clear the threshold. If
  structural diversification is wanted earlier, the thresholds to examine are
  `deltaTargetInit`/`deltaStep`/`deltaMin` and `distanceC3` — measured at 40
  Generations first, since speciation usually shows up late rather than never.
- **Alive time sits on the 60-second Wave clock.** A Ship that survives the Wave
  is not scored for surviving the next one until the field is cleared, so
  `waveTimeLimit` and `waveGrowth` are the two constants that decide how much of
  the curve is "survives" rather than "clears".
- **Nothing here has been re-measured against the browser rendition.** The
  browser's numbers are gone with the old save format and were never meant to be
  reproduced bit for bit (ADR 0005).
