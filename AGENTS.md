# crosgha

Rust CLI that detects smells in GitHub Actions workflows and fixes them deterministically, preserving comments and
formatting. The CLI recursively scans workflows, parses YAML with `yamlpath`, and validates top-level `on` and `jobs`.
The first rule detects and fixes checkout steps that retain credentials. Four opt-in rules detect mutable references,
missing job timeouts, implicit token permissions, and inherited reusable-workflow secrets without guessing fixes.
JSON/SARIF output and release-backed Action execution are still pending.
`crosgha` stands for aCtions Repair Of miSconfigured GitHub Actions.

This repo is published on its own. When it is checked out inside the research workspace, `CLAUDE.workspace.md` is a
local-only symlink to the workspace guide, which covers the tool map, curated smell taxonomy, and reproduction harness.
It is listed in `.git/info/exclude` and never committed.

## Commands

| Task | Command |
|---|---|
| One-time hook setup after cloning | `make setup` (sets `core.hooksPath=.githooks`) |
| Build / run | `cargo build --release` · `cargo run` |
| Everything the pre-push hook checks | `.githooks/pre-push` (fmt, clippy `-D warnings`, tests, dogfood) |
| Release plan preview (offline) | `.tools/bin/dist plan` |

## Rules

## Tests

1. Never write unit tests after writing the code.
2. End-to-end (E2E) tests are the main way to test; use them to check that complex
   features work. Each E2E test produces an artifact that can be checked and reproduced.
3. For complex features, use realistic E2E scenarios of medium or high complexity, not
   only the simplest successful case.
4. Testing a system in isolation: first list every way it could fail, then write the code.
5. No tautological tests (confirming what the code already says) and no tests that only
   detect whether code changed.
6. Bug fixes get a regression test only when existing behavior tests leave a real gap.

1. **Independence.** Allowed dependencies are infrastructure crates only, pinned exactly:
   - `github-actions-models`, `yamlpath`, `yamlpatch`, `github-actions-expressions` at `=1.30.1`
   - generic crates such as clap and serde

   Never embed, vendor, or shell out to another detector (zizmor's audit crate, actionlint, poutine, …) for detection or fix validation. Rule *concepts* come from the literature and tool docs, cited in code comments. Our own untrusted-context list is derived from GitHub's docs. No third-party linter runs on this repo either: crosgha checks our own workflows in CI and the pre-push hook.
2. **Fix validation is ours.** A fix is accepted only when all three hold:
   - the patched file re-parses
   - the targeted smell is gone and no new smell appeared
   - only the intended YAML nodes changed
3. **SemVer from 0.1.0 through Conventional Commits.**
   - The `commit-msg` hook enforces `type(scope)!: subject`.
   - Never bump `version` or edit `CHANGELOG.md` by hand; release-please does both. Before 1.0, breaking changes bump the minor version (`bump-minor-pre-major`).
   - Breaking changes use `!` or a `BREAKING CHANGE:` footer.
4. **Dogfood.** Our workflows must stay free of the smells we detect:
   - actions pinned to a 40-char SHA with a `# vX.Y.Z` comment
   - minimal `permissions`
   - `timeout-minutes` on every job
   - `concurrency` in CI
   - `persist-credentials: false`

   `.githooks/pre-push` and CI run crosgha on `.github/workflows` with all four opt-in policies enabled. Accepted exceptions need a written reason in a comment next to the exception.
5. **Toolchain.** Rust is pinned only in `mise.toml` (1.97), matching the workspace. CI installs it with `jdx/mise-action`, whose mise binary is pinned by version and sha256. Local CLIs (e.g., `dist`) live in `.tools/` (gitignored), installed with `cargo install --root .tools <crate> --locked`.
6. **Tests.** Every rule ships with before/after YAML fixtures and a test asserting the exact fixed output. `tests/cli.rs` runs the real binary end to end.

## Layout

```
src/main.rs                 CLI entry point
src/cli.rs                  CLI options, orchestration, exit status
src/workflow.rs             YAML parsing and workflow validation
src/rules/                  Rule detection and fix proposals
src/fix.rs                  Shared fix validation
src/filesystem.rs           Workflow discovery and verified writes
src/report.rs               Text output and planned JSON/SARIF formats
src/policy.rs               Severity and failure threshold
action.yml                  Composite Action metadata and inputs
action/entrypoint.sh        Action input guard; execution scaffold
docs/ARCHITECTURE.md        Module interfaces and Action flow
tests/cli.rs                end-to-end tests of the built binary
tests/fixtures/              workflow inputs for CLI checks
.githooks/                  commit-msg (Conventional Commits), pre-push (full check)
.github/workflows/          ci.yml, release-please.yml (tags, draft release, dispatches release.yml), release.yml (cargo-dist, workflow_dispatch only)
release-please-config.json  release PR + changelog config (manifest: .release-please-manifest.json)
dist-workspace.toml         cargo-dist target: x86_64-unknown-linux-gnu only
mise.toml                   Rust version (the only pin; used locally and in CI)
ROADMAP.md                  plans: GitHub Action packaging (composite, SHA-verified binary, SARIF)
```

## Going live (not done yet)
1. Create the GitHub repo as **private** (a study comes out of it; the author creates it, not agents) and push `master`.
2. Enable "Allow GitHub Actions to create and approve pull requests".

No PAT or secret is needed: `release-please.yml` dispatches `release.yml` with the default `GITHUB_TOKEN`.
