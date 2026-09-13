# ADR 0008 — The Competence Gate Watches the Headline Pair

The program's headline competence number is the Population's mean Waves at a fixed Generation, with median alive time as the tie-break (ADR 0007). The Gate answers one question — has this lineage stopped improving? — and it must answer it about that same number, or its verdict misleads: it would trip on the tie-break while the headline still climbed, and stay silent while the headline did not.

Stagnation is therefore judged on the tie-broken pair, keeping the Gate's existing shape: a window of Generation pairs and a run of stagnant Generations. While nobody has cleared a Wave the mean is exactly zero, so the rule degenerates to the alive-time comparison it replaces and the early Generations read much as they did before. The old best-ever escape hatch is gone: it answered a third question — whether any member beat an all-time record in alive time, best Wave or best Asteroid points — so it could hold the Gate silent while the headline stalled, the exact failure this ADR exists to prevent.

## Considered Options

- **Leave the Gate on alive time** — rejected: a health check on the tie-break can trip on noise unrelated to the metric, and the HUD would then report a stagnant lineage while the program's own number improved.
- **Trip when either metric stagnates** — rejected: more noise, and a trip would no longer name what stopped improving.
- **Keep the best-ever escape hatch** — rejected: a record in any banked metric is neither the headline nor its tie-break, so the escape could hold the Gate silent through a stalled headline — the failure this ADR exists to prevent.
- **Retire the Gate during the program** — rejected: during long runs it is the only signal that says a lineage is done.

## Consequences

ADR 0003's shaping is untouched; only its Gate mechanism changes. No Gate constant moved with the rule, and `TUNING.md` entry #7 records that: `MEDIAN_WINDOW` still counts Generations, `STAGNATION_LIMIT` still counts a run of stagnant Generations, and `STAGNATION_RATIO` still scales median alive time in seconds — now only when mean Waves tie. The primary comparison needs no ratio, because mean Waves is itself a fraction of a Wave.
