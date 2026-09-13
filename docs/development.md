# Rust development tooling

## Setup

Install Rust through rustup and install Node.js 24 or newer for the Git hook.
On macOS, Xcode Command Line Tools are also required (`xcode-select --install`).

```sh
rustup show active-toolchain # installs the repository's pinned toolchain/components
npm ci                      # installs Prettier/lint-staged and activates Husky
npm run tools:install       # nextest, cargo-deny, cargo-machete, cargo-llvm-cov
```

The application still builds and runs with Cargo alone. Node is for the commit
hook and convenient command aliases. The optional Cargo tools are required for
dependency checks, nextest, and coverage, but not for ordinary commits.

`rust-toolchain.toml` pins Rust 1.98.1 so local and CI formatting/lints agree.
It also installs rustfmt, Clippy, rust-src, rust-analyzer, and LLVM coverage tools.
Configure your editor to use rust-analyzer from rustup; enable Clippy as its
check command if desired. Both crates continue to forbid unsafe code.

The workspace's minimum supported Rust version is 1.89, matching the existing
cosmic-text dependency's requirement. CI separately checks compilation with
1.89.0. Change the pinned toolchain deliberately and run the complete checks
when updating it; a newer Clippy may introduce new warnings.

## Everyday commands

| Command                                                                 | Checks or output                                                                    |
| ----------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `npm run check`                                                         | Rust formatting, strict Clippy, all workspace tests/doctests, and rustdoc warnings  |
| `cargo fmt --all`                                                       | Format Rust files                                                                   |
| `cargo typecheck`                                                       | Compile-check every workspace target and feature with the lockfile                  |
| `cargo lint`                                                            | Clippy on every workspace target and feature; warnings fail                         |
| `cargo test-all`                                                        | Standard Cargo test runner, including doctests                                      |
| `npm run test:fast`                                                     | Nextest for workspace tests, then Cargo for doctests                                |
| `cargo nextest run -p neuroarena-sim`                                   | Simulation tests without a GPU                                                      |
| `npm run docs:check`                                                    | Generate workspace API docs; documentation warnings fail                            |
| `npm run deps:check`                                                    | Advisories, licenses, dependency sources, wildcard constraints, unused dependencies |
| `npm run coverage`                                                      | Workspace HTML coverage at `target/llvm-cov/html/index.html`                        |
| `cargo +1.89.0 check --workspace --all-targets --all-features --locked` | Minimum Rust version check after installing that toolchain                          |

The pre-commit hook runs staged-file Prettier, checks Rust formatting, runs
Clippy, and runs the full Cargo test suite. Clippy includes compilation checking,
so a separate typecheck would duplicate work in the hook. Rust formatting is
checked rather than automatically rewriting or staging whole files: this keeps
partially staged Rust edits intact. Run `cargo fmt --all`, review, and stage the
changes when formatting fails.

The workspace test suite includes GPU golden-frame tests. They require a working
GPU adapter and intentionally fail if one is unavailable. Use the simulation-only
command for a display/GPU-free development loop; it does not replace the full
workspace checks. Never regenerate goldens just to make a tooling check pass.

Nextest's CI profile completes the suite after failures and writes JUnit results.
Tests are not retried. A test gets a slow-test notice every 60 seconds and is
terminated after five such periods. Doctests run separately because the standard
Cargo runner remains responsible for them.

## CI and dependency policy

GitHub Actions runs formatting, Clippy, documentation, all workspace tests, and
coverage on macOS. Linux runs the portable simulation tests. CI uploads JUnit
results and LCOV coverage artifacts without needing an external service token.
Coverage is reported without an arbitrary percentage gate.

Dependency checks run on pull requests, pushes to main, and weekly so newly
published advisories are checked even without code changes. `deny.toml` evaluates
both Apple Silicon and Intel macOS dependency graphs. It allows the permissive
licenses used by those graphs, rejects unknown registries/Git sources and wildcard
versions, and checks advisories. Duplicate transitive versions are warnings:
the graphics/font stack currently needs them. Direct unmaintained dependencies
are in scope; known unsoundness is checked throughout the graph. No advisory
exceptions are configured.

Dependabot proposes weekly Cargo, npm, and GitHub Actions updates. Third-party
Actions are pinned to commit hashes. CI uses read-only repository permissions,
timeouts, Cargo caching, and cancellation of superseded pull-request runs.

The evidence and selection rationale are in
[the Rust tooling research notes](research/rust-tooling.md).
