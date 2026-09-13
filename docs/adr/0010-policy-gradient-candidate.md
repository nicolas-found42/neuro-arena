# ADR 0010 — A Policy-Gradient Candidate in Its Own Crate

A model-free policy-gradient agent (PPO) joins the programme as a Candidate, built in its own crate against the `sim` public API rather than inside the Genome path. The evolved controller, the determinism contract and the save format are therefore untouched by it, and if the Candidate is abandoned its crate is deleted and nothing else is disturbed. It observes the same Sensorium the Ship does — memory channel included, so both candidates act on identical information — and it acts through the same four Bernoulli controls: turn left, turn right, thrust, fire.

Its reward is competence-aligned: survive, clear Asteroids, advance Waves. It deliberately does not optimise the shaped Fitness that breeds the evolved path, and the difference is reported rather than hidden. The shaped score exists to steer a population search that has Species and a novelty archive, and its novelty term is not definable for a single agent at all. Both Candidates are judged on the same competence metric, by the same sweep, at an equal simulation-step budget with wall-clock reported separately — the arm spends a currency this project has never paid, and the account should show it.

Why it exists: round 2 of the design session allowed weight training on the premise that gradients need a differentiable simulator. That premise was wrong — PPO backpropagates through the policy network and never differentiates the environment — so the question became cost rather than feasibility, and the owner chose to pay it. The surveys rank gradient RL last, behind selection signal, novelty and quality-diversity, so this Candidate is built now and judged in its turn: after the Sensorium Candidate, not instead of it.

## Considered Options

- **Training the Genome's weights in place** — rejected: it entangles gradient machinery with the determinism contract and the save format, and a PPO result would then depend on the very plumbing it is meant to be compared against.
- **Building it later, as a yardstick only** — rejected by the owner: a head-to-head number under the same protocol is worth the build now.
- **The shaped Fitness as the reward** — rejected: near apples-to-apples on the objective, but that shaping was tuned for a population search, so a gradient learner exploiting it differently would say more about the shaping than about the algorithm.

## Consequences

The workspace gains a third crate, and the sweep binary must compare across two different trainers, so its output gains the per-run simulation-step count that the equal-budget rule needs. `sim` changes for none of this. The Candidate's result is the first direct evidence on whether gradients or evolution suit this Arena better, and it arrives as one Candidate among several rather than as a verdict.
