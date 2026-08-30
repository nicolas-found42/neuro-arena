# ADR 0003 — Shaped Fitness with Separate Competence Gate

Fitness that breeds the next Generation is shaped — alive time plus movement, bullet cost, action-entropy, and a novelty-archive bonus decaying over 60 generations to a 0.2 floor — while raw Competence (alive time, Wave reached, Asteroid points) is tracked separately via the gate and shown in the HUD.

Pure survival or pure score breeds degenerate spinners and campers; sensorimotor-entropy (arXiv:1006.4959, 2608.12534) and behavior-descriptor novelty with decaying exploration (arXiv:1902.03142, 2209.03618) sustain diverse use of all four controls and early exploration without masking real skill. The split lets `verify.mjs` and `EvolutionRunner.gate` judge learning on unshaped metrics while selection still exploits shaping. Rejected pure-survival and score-only fitness for collapsing to local minima observed in tuning probes.

Consequences: `World` banks unshaped stats; `Evaluation` adds entropy in-World and novelty at banking time; `Population.history` and `Chart` plot shaped Fitness while the HUD gate watches raw competence; tuning decisions cite research in `js/config.js`.
