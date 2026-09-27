# crosgha

Rust CLI that detects smells in GitHub Actions workflows and fixes them deterministically, preserving comments and
formatting. The CLI recursively scans workflows, parses YAML with `yamlpath`, and validates top-level `on` and `jobs`.
46 rules cover every tag of the curated taxonomy: 38 per workflow, 3 per repository, and 5 that query the GitHub API.
Only `persist-credentials` has a verified fix so far. Personas, ignore comments, `zizmor.yml` (including per-rule
options), and `crosgha migrate` follow zizmor so users can switch tools without rewriting configuration; reports name
the equivalent zizmor or actionlint rule. Output is text, JSON, SARIF, GitHub annotations (the default under `GITHUB_ACTIONS=true`), or brief agent lines. The Action runs in report mode with a sha256-verified release
binary; PR and auto-merge modes are still pending.
`crosgha` stands for aCtions Repair Of miSconfigured GitHub Actions.

This repo is published on its own. When it is checked out inside the research workspace, `CLAUDE.workspace.md` is a
local-only symlink to the workspace guide, which covers the tool map, curated smell taxonomy, and reproduction harness.
It is listed in `.git/info/exclude` and never committed.

## Commands

| Task | Command |
|---|---|
| One-time hook setup after cloning | `make setup` (sets `core.hooksPath=.githooks`) |
| Build / run | `cargo build --release` · `cargo run` |
| Everything the pre-push hook checks | `.githooks/pre-push` or `make check` (fmt, clippy `-D warnings`, tests, dogfood) |
| Rule catalog (id, tag, severity, persona, scope) | `cargo run -- --list-rules` (`-f json` adds summaries and equivalents) |
| Release plan preview (offline) | `.tools/bin/dist plan` |

## Rules

1. **Independence.** Allowed dependencies are infrastructure crates only, pinned exactly:
   - `github-actions-models`, `yamlpath`, `yamlpatch`, `github-actions-expressions` at `=1.30.1`
   - generic crates such as clap and serde

   Never embed, vendor, or shell out to another detector (zizmor's audit crate, actionlint, poutine, …) for detection or fix validation. Rule *concepts* come from the literature and tool docs, cited in code comments. Our own untrusted-context list is derived from GitHub's docs. No third-party linter runs on this repo either: crosgha checks our own workflows in CI and the pre-push hook. Compatibility is allowed: reading another tool's config format, accepting its rule IDs as aliases, and rewriting its Action step in `migrate`. Running or porting its detection logic is not.
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

   `.githooks/pre-push` and CI run crosgha on `.github/workflows` with `--offline --persona pedantic`. Accepted exceptions need a written reason in a comment next to the exception.
5. **Toolchain.** The Rust toolchain is pinned only in `mise.toml` (1.97), matching the workspace; `rust-version` in `Cargo.toml` declares the same minimum. `.githooks/pre-push` re-runs itself under `mise exec` when mise is not active in the calling shell (e.g., `gh repo create --push`, IDEs), so it never falls back to rustup's default toolchain. CI installs it with `jdx/mise-action`, whose mise binary is pinned by version and sha256. Local CLIs (e.g., `dist`) live in `.tools/` (gitignored), installed with `cargo install --root .tools <crate> --locked`.

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
7. Every rule ships with YAML fixtures. Workflow rules live in `tests/fixtures/rules/`,
   online rules live in `tests/fixtures/online/`, and repository rules have a named test in
   `tests/cli.rs` or `tests/rules.rs`. `tests/rules.rs` checks each `.before.yml` against its
   `# expect: <rule>` markers and fails when a registered rule has no fixture. The hand-written
   `.clean.yml` applies the remediation and must not be reported. `.after.yml` is never written by
   hand: it is the exact `--fix` output, regenerated with `CROSGHA_BLESS=1 cargo test --test rules`
   and reviewed in the diff; it is absent when `--fix` leaves the file unchanged.

## Layout

```
src/main.rs                 CLI entry point
src/cli.rs                  CLI options, orchestration, exit status
src/workflow.rs             YAML parsing and workflow validation
src/rules/                  Rules by dimension, shared traversal (common.rs), repository and online rules
src/fix.rs                  Shared fix validation
src/filesystem.rs           Workflow discovery and verified writes
src/report.rs               Text output, rule catalog, and planned JSON/SARIF formats
src/policy.rs               Personas, severity, settings, and failure threshold
src/config.rs               crosgha.yml / zizmor.yml discovery and rule settings
src/github.rs               Cached read-only GitHub REST client for online rules
src/migrate.rs              `migrate --from zizmor|actionlint` step rewrites
src/hook.rs                 `hook install|uninstall`: the git pre-push hook
action.yml                  Composite Action metadata and inputs
action/entrypoint.sh        Action: input checks, verified release install, one run, outputs
docs/ARCHITECTURE.md        Module interfaces and Action flow
tests/cli.rs                end-to-end tests of the built binary
tests/rules.rs              fixture-driven rule checks, fake GitHub API server
tests/action.rs             Action entry point against a local release server
tests/fixtures/             rules/, online/ (+ api.json), repos/, migrate/, and CLI inputs
.githooks/                  commit-msg (Conventional Commits), pre-push (full check)
.github/workflows/          ci.yml, release-please.yml (tags, draft release, dispatches release.yml), release.yml (cargo-dist, workflow_dispatch only)
release-please-config.json  release PR + changelog config (manifest: .release-please-manifest.json)
dist-workspace.toml         cargo-dist target: x86_64-unknown-linux-gnu only
mise.toml                   Rust version (the only pin; used locally and in CI)
ROADMAP.md                  plans: GitHub Action packaging (composite, SHA-verified binary, SARIF)
```

## Hosting
- GitHub repo: `meiazero/crosgha`, **private** (a study comes out of it), default branch `master`, remote `origin`.
- "Allow GitHub Actions to create and approve pull requests" is enabled, so release-please can open its release PR.

No PAT or secret is needed: `release-please.yml` dispatches `release.yml` with the default `GITHUB_TOKEN`.
