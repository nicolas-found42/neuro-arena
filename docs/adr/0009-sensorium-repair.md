# ADR 0009 — Sensorium Repair: Thirteen Rays and Three Threat Slots

The Sensorium goes from 21 inputs to 34. Nine Sensor Rays become thirteen at 27.7° spacing, which lifts the guaranteed detection radius from 2.92·r to 4.18·r — for Large / Medium / Small Asteroids, 111 / 61 / 32 px becomes 159 / 88 / 46 px. The 500 px `RANGE` was never reachable: a rock must lie within 2.92·r of a ray axis to be seen at all, so a Medium Asteroid was invisible outside roughly 61 px, about twelve simulation ticks of warning at a 300 px/s closing rate. The single tracked Asteroid becomes three Threat Slots, each carrying bearing, closeness, closing rate, lateral rate and size; the stale second-nearest closeness input is subsumed and removed.

Why: the best-measured result in the sensorium survey is that removing velocity from an observation space significantly degrades dynamic obstacle avoidance and that recurrence does not reliably substitute for it (arXiv:2112.12465). Closeness for the second and third nearest rocks is a weaker substitute than their actual motion, and angular resolution — not `RANGE` — is what bounds what the pilot can react to. Beam placement beats uniform spacing at a fixed budget (arXiv:2201.03860), but the budget here is open, and uniform spacing is the version whose worst case can be stated exactly.

## Considered Options

- **Seventeen rays** — rejected for now: 5.52·r doubles the warning window again, at eight more inputs and forty more founding connections. The second-order gain waits until the first-order one is measured.
- **Nine rays, placed non-uniformly** — rejected: it is neutral in input count but there is no measured prior for where an Asteroids pilot needs density, and the score is dominated by whichever arc the guess starves.
- **Per-ray hit radius instead of more rays** — rejected: it adds identity without adding resolution, and leaves every mid-gap direction exactly as blind as it is today.
- **One slot with cross-tick association** — rejected: it fixes identity discontinuity while leaving the second and third rocks without velocity, and it puts state into a sensing path that is currently pure.

## Consequences

The founding Genome grows from 26 nodes / 105 connections to 34 × 5 = 170 connections, and the save format takes the version bump ADR 0007 already decided — older files are refused rather than guessed at. The Network panel's input column draws 34 dots at a tighter pitch; its twelve-hidden-node cap is untouched. Because the sensorium widens, everything downstream of it — memory, recurrence, the designed Body Plan arm — inherits the new shape and is measured against this candidate's result, not against today's baseline.
