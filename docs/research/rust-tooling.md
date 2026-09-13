# Rust tooling research

Researched 2026-09-12. Started with awesome lists, then inspected primary GitHub
examples and tool documentation. Research and implementation were performed
without subagents.

## Discovery and examples

- [Awesome Rust](https://github.com/awesome-rust-com/awesome-rust) was the first
  discovery source. Its development-tools section lists Clippy, and its game
  development section points to Bevy. The older
  [rust-unofficial list](https://github.com/rust-unofficial/awesome-rust) was also
  checked. Lists provided discovery; actual configuration decisions came from
  the projects below.
- [Bevy CI](https://github.com/bevyengine/bevy/blob/main/.github/workflows/ci.yml)
  separates build/tests from lint checks, installs rustfmt and Clippy, runs on
  multiple operating systems, and uses read-only permissions and concurrency
  cancellation. This is a useful game-development example. We adopt the split
  and operational controls, scaled to macOS plus the portable simulation on Linux.
- [Ratatui CI](https://github.com/ratatui/ratatui/blob/main/.github/workflows/ci.yml)
  covers formatting, Clippy, cargo-deny, cargo-machete, coverage, documentation,
  and minimum/stable Rust checks. It uses cargo-llvm-cov and a shared Cargo cache.
  We adopt these check categories without its large feature/backend matrix,
  nightly unused-dependency analysis, or external coverage service.
- [cargo-deny's own policy](https://github.com/EmbarkStudios/cargo-deny/blob/main/deny.toml)
  provides a concrete example of target filtering, advisory scope, explicit
  license allowances, dependency bans, and source restrictions. Our policy uses
  this structure with macOS targets and the actual licenses in this workspace.
  Duplicate versions remain warnings because the graphics dependencies need them.

## Tool decisions

| Tool                  | Why included                                                                                                     | Primary reference                                                                                                                       |
| --------------------- | ---------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| rustup toolchain file | Same compiler, formatter, and Clippy locally and in CI; editor and LLVM components installed together            | [rustup overrides](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file)                                                |
| rustfmt and Clippy    | A consistent formatting baseline and actionable compiler/lint failures                                           | [Clippy](https://github.com/rust-lang/rust-clippy)                                                                                      |
| cargo-nextest         | Separate test processes, shared CI profile, slow-test handling, JUnit output; Cargo doctests retained separately | [nextest configuration](https://nexte.st/docs/configuration/), [usage](https://nexte.st/docs/running/)                                  |
| cargo-deny            | One dependency-policy command instead of overlapping advisory scanners                                           | [configuration](https://embarkstudios.github.io/cargo-deny/checks/cfg.html)                                                             |
| cargo-machete         | Fast unused-dependency detection; metadata mode understands crate library names                                  | [cargo-machete](https://github.com/bnjbvr/cargo-machete)                                                                                |
| cargo-llvm-cov        | Local HTML and CI LCOV coverage with Rust's LLVM tooling                                                         | [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)                                                                             |
| Dependabot            | Weekly update proposals for Cargo, hook tooling, and pinned Actions                                              | [GitHub configuration reference](https://docs.github.com/en/code-security/reference/supply-chain-security/dependabot-options-reference) |

These are choices for this workspace, not a claim that every Rust project needs
the same tools. No custom task-runner crate is needed: Cargo aliases and the
already-present npm scripts expose the commands. Miri, feature-permutation
testing, fuzzing, mutation testing, and benchmarks are deferred until there is a
specific invariant or workload to target. Both crates already forbid unsafe
code, neither defines a feature matrix, and inventing a benchmark workload would
not validate this tooling setup.

## Baseline findings

- Rust sources had not been normalized with rustfmt; enabling the gate requires
  a one-time formatting baseline.
- Strict Clippy found redundant borrows/casts, an unnecessary unwrap, manual
  iteration/division patterns, and fixture dead-code warnings across separate
  integration-test binaries. Fixes preserve behavior; narrow documented lint
  exceptions keep the shared test fixtures and paired `rgb`/`rgba` API intact.
- Cargo metadata declared Rust 1.82 for the workspace, but the already-locked
  cosmic-text 0.19 requires 1.89. The minimum is corrected and checked separately.
- The renderer tests explicitly require a GPU. Full workspace tests and coverage
  therefore use macOS runners; Linux exercises the independent simulation crate.
- The locked macOS graph passed advisory and license checks. Duplicate dependency
  versions remain visible warnings, not blanket ignored advisory exceptions.

## Local verification

- `npm run check`: formatting, strict Clippy, 88 unit/integration tests, one
  doctest, and strict rustdoc all passed. GPU golden tests used the existing image.
- Nextest with the CI profile: 88 passed, zero skipped; JUnit artifact produced.
- Full workspace compile-check with Rust 1.89.0: passed.
- cargo-deny: advisories, bans, licenses, and sources passed; duplicate-version
  warnings remain. cargo-machete found no unused dependencies.
- Workspace coverage run passed and produced HTML and LCOV reports: 58.58% line
  coverage. This includes the interactive application, not just the simulation.
- actionlint and TOML validation passed. An isolated hook smoke test verified
  that a failure at each of the four gates stops subsequent commands.

GitHub-hosted jobs have not been executed locally; their runner/GPU behavior must
still be confirmed by the first remote CI run. Cargo also reports an existing
future-compatibility warning in the transitive `block` 0.1.6 dependency.
