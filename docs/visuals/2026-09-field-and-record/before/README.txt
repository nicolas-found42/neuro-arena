Provenance, per file. Read this before quoting any pair as a controlled
comparison.

before/ — captured from a clean git worktree at HEAD ab988f1 ("the observatory,
legible"), release build. after/ — captured from the working tree of this pass.
Both with:

  cargo build --release -p neuroarena-app --example visual_probe
  visual_probe <out.png> <w> <h> <steps|impact> <dpr> <completed> <member> [dense|still|rays]

Seed 2026, population 100, member 0, dpr 1 except retina.png (dpr 2).

MATCHED PAIRS — identical arguments on both sides, differing only by this pass:
  live, dense, impact, min, retina, g150, wide

NOT MATCHED, and why:
  native-*.png               Screen captures of the release binary
                             (`screencapture -x -o -R 40,30,1440,900`), not probe
                             output. before/native-960x640.png has no after
                             counterpart; the two native frames were taken at
                             different run ages.
  after/trace.png            No before counterpart: captured at 960 steps
                             completed 150 specifically because that is where the
                             ribbon is long enough to read, a state the pre-pass
                             harness was not driven to.
  after/reduced-motion.png   No before counterpart: the `still` state did not
                             exist in the pre-pass harness, and the pre-pass
                             renderer had no Motion control to exercise.

TWO DIFFERENCES BEYOND THE APPLICATION, both in the harness:
  1. The Ray overlay became an opt-in trailing word (`rays`) instead of always
     on, matching the app, which defaults it off. The same edit was applied to
     the pre-pass worktree's copy of visual_probe.rs, so both sides draw the
     default frame — but it is a harness edit, not an application one.
  2. The after-probe evaluates a full Generation so the Record has a Cohort to
     draw; the before-probe did not. The arena geometry is unaffected (the same
     member, seed and step count), but the sidebar is not a pure before/after
     of the same computation.
