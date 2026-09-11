# LSP (Rust) in this repo — why `lsp` said “No language server found”, and the fix

## Symptom

`lsp` actions against `sim/src/competence.rs` — e.g. a field rename — returned
`No language server found for this action` with `details.success: false`.
The same branch yields `No language server found for this file` for single-file
actions in current builds; both mean “no configured server matches this file”
(`packages/coding-agent/src/lsp/tool.ts`, lines ~884 and ~989/~1104).

## Root cause

rust-analyzer **is** installed, present, and resolvable on this machine. The
defect is omp’s *server list for this project*:

1. `loadConfig(cwd)` seeds the built-in `defaults.json`, merges every `lsp.*`
   config file, then keeps only servers whose **root markers exist in the cwd**
   and whose binary resolves — `config.ts` (`hasRootMarkers` … `resolveCommand`).
   rust-analyzer’s markers are exactly `["Cargo.toml", "rust-analyzer.toml"]`
   (`defaults.json`), and marker detection is one-level and cwd-only: it never
   walks parent directories (omp://lsp-config.md).
2. `getConfig(cwd)` caches that result in a module-level `configCache` Map, so the
   first observation lasts the life of the omp process (omp://tools/lsp.md, Notes;
   issue #3546 — fixed by #3550, long before the installed 18.1.17).

This omp process has been up ~7h; the root `Cargo.toml` is ~1h old, so the cached
observation predates the Rust workspace:

| Evidence | Value |
| --- | --- |
| `lsp {action:"status"}` (run by Main) | the 7 dotfile-marked servers only — no rust-analyzer at all |
| `~/.omp/agent/cache/composer/7856dfa817749db8/lsp-servers.json` | the identical 7, mtime 7h ago (dir id = this project’s mux daemon scope) |
| `~/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/components` | `rust-analyzer-preview-aarch64-apple-darwin` (installed) |
| `~/.cargo/bin/rust-analyzer`, `~/.rustup/toolchains/stable-aarch64-apple-darwin/bin/rust-analyzer` | both present |
| other projects’ `lsp-servers.json` (e.g. `0f9a56524b8e78aa`) | list `rust-analyzer` for `.rs` → the binary resolves on PATH |

The seven servers appear because their own markers are `[".git"]` or
`["package.json", ".git"]` and this repo has `.git`; the Rust server’s marker is
`Cargo.toml`, which did not exist when the list was computed.

**Not the cause:** the `omp.lsp.mux` daemon (transport only — omp://tools/lsp.md
§ Side Effects), and `~/.omp/agent/lsp.json` (a week-old user override that also
omits rust-analyzer — but built-in defaults are always the merge base, so a config
file that omits a server does not remove it).

## Fix

```text
lsp {action:"reload", file:"*"}
```

Workspace reload deletes the per-cwd `configCache` entry and re-reads config from
disk (omp://tools/lsp.md § reload; tool.ts reload branch), so the now-present
`Cargo.toml` is seen and rust-analyzer is added and started.

If `status` still omits rust-analyzer afterwards, the binary did not resolve inside
that process — pin it explicitly in the highest-precedence project config
(omp://lsp-config.md) by creating `<repo>/.omp/lsp.json`:

```json
{ "servers": { "rust-analyzer": { "command": "/Users/Nicolas/.cargo/bin/rust-analyzer" } } }
```

then `lsp {action:"reload", file:"*"}` again. Restarting the omp process is the
guaranteed fallback, since `configCache` lives in-process (issue #3546).

No installation is required here. On a machine that lacks it:
`rustup component add rust-analyzer` (rustup book, Components).

## Verification

1. `lsp {action:"status"}` → must list `rust-analyzer` (`(configured, not started)`
   or a live status).
2. `lsp {action:"capabilities", file:"sim/src/competence.rs"}` → capability JSON.
3. `lsp {action:"rename", file:"sim/src/competence.rs", line:18, symbol:"alive_time",
   new_name:"alive_seconds", apply:false}` → preview of a `WorkspaceEdit` spanning the
   workspace (line 18 is `pub alive_time: f64,`).
4. Repeat without `apply` (it defaults to true) to write the rename.

## Renames

Once the server is live, use `lsp rename` rather than text edits for cross-file Rust
renames: it applies the server’s `WorkspaceEdit`, and the client roots the server at
the cwd, whose `Cargo.toml` is a `[workspace]` with `members = ["sim", "app"]`
(tool.ts § rename; issue #1648 maintainer note that `rootUri = cwd`). `rename`
requires `symbol` whenever `line` is given on project-aware servers, and `apply`
defaults to true — pass `apply:false` to preview. Use `rename_file` for moving or
renaming whole files.

While no Rust server is available, `lsp {action:"diagnostics", file:"*"}` still works:
workspace mode shells out to `cargo check --message-format=short`
(omp://tools/lsp.md § diagnostics).

## Sources

- Harness docs: `omp://tools/lsp.md`, `omp://lsp-config.md`.
- Harness source @ can1357/oh-my-pi `main`: `packages/coding-agent/src/lsp/config.ts`,
  `defaults.json`, `tool.ts`.
- Issues: can1357/oh-my-pi #3546 (configCache never invalidated), #1648 (cwd-only marker
  detection), #3550 (fix).
- Local: rustup `components` file and `~/.rustup/settings.toml`; composer
  `lsp-servers.json` caches; `~/.omp/agent/lsp.json` (mtime 1 week); `~/.cargo/env`
  sourced from `~/.zshenv`; `/opt/homebrew/Cellar/omp/18.1.17` (installed 2026-09-10).
