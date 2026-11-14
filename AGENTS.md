# crosgha

Rust CLI and composite GitHub Action that detect smells in GitHub Actions workflows and fix them deterministically,
preserving comments and formatting. `crosgha` stands for aCtions Repair Of miSconfigured GitHub Actions.

- **Scan:** finds workflows directly inside `.github/workflows`, parses YAML with `yamlpath`, and validates top-level `on` and `jobs`.
- **Rules:** 48 cover every tag of the curated taxonomy: 39 per workflow, 4 per repository, and 5 that query the
  GitHub API.
- **Fixes:** only `persist-credentials`, `removed-feature` (setup-java distributions), `deprecated-command`
  (workflow commands to environment files), and `embedded-script-lint` (quotes and braces environment file paths,
  appends instead of overwriting) have verified fixes so far. `persist-credentials` stays unfixed when `true` is
  explicit, a later step pushes with the token, or checkout predates v2. `deprecated-command` rewrites only whole
  `echo` lines in bash or sh steps, and `embedded-script-lint` only `>`/`>>` redirects whose target is an
  environment file (docs/fixes.md lists what each leaves).
- **zizmor compatibility:** personas, ignore comments, `zizmor.yml` (including per-rule options), and
  `crosgha migrate` follow zizmor so users can switch tools without rewriting configuration. Reports name the
  equivalent zizmor or actionlint rule.
- **Output:** text, JSON, SARIF, GitHub annotations (the default under `GITHUB_ACTIONS=true`), or brief agent lines.
- **Entry points:** the CLI, `crosgha hook install` (git pre-push hook), and the Action in report mode with a
  sha256-verified release binary. PR and auto-merge modes are still pending (issues #72 and #73).

This repo is published on its own. When it is checked out inside the research workspace, `CLAUDE.workspace.md` is a
local-only symlink to the workspace guide, which covers the tool map, curated smell taxonomy, and reproduction harness.
It is listed in `.git/info/exclude` and never committed.

## Commands

| Task | Command |
|---|---|
| One-time hook setup after cloning | `make setup` (sets `core.hooksPath=.githooks`) |
| Build / run | `cargo build --release` · `cargo run` |
| Format Rust | `cargo fmt-style` (never plain `cargo fmt`: it skips the options of rule 6) |
| Everything the pre-push hook checks | `make check` (whitespace, `cargo fmt-style --check`, clippy `-D warnings`, tests, dogfood, cargo-deny). When a push changes no path listed in `ci.yml`'s `paths`, CI does not run and the hook checks only whitespace |
| Rule catalog (id, tag, severity, persona, scope) | `cargo run -- --list-rules` (`-f json` adds summaries, equivalents, and `configurable`) |
| Release plan preview (offline) | `.tools/bin/dist plan` |

## Rules

1. **Independence.** Allowed dependencies are infrastructure crates only:
   - YAML crates, pinned exactly with `=` in `Cargo.toml`: `yamlpath`, `yamlpatch`, `yaml_serde`
   - generic crates, with caret requirements: `anstream`, `anstyle`, `clap`, `serde`, `serde_json`, `ureq`
   - allowed but unused, pinned with `=` if ever added: `github-actions-models`, `github-actions-expressions`

   Never embed, vendor, or shell out to another detector (zizmor's audit crate, actionlint, poutine, …) for detection or fix validation. Rule *concepts* come from the literature and tool docs, cited in code comments. Our own untrusted-context list is derived from GitHub's docs. No third-party linter runs on this repo either: crosgha checks our own workflows in CI and the pre-push hook. cargo-deny is allowed because it checks dependencies, not workflows: RustSec advisories, licenses, sources, and a ban on detector crates (`deny.toml`). `clippy.toml` enforces rules 1 and 2 (and part of rule 6): only `src/hook.rs` runs a process, and only `filesystem::replace` writes a workflow. Compatibility is allowed: reading another tool's config format, accepting its rule IDs as aliases, and rewriting its Action step in `migrate`. Running or porting its detection logic is not.
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
5. **Toolchain.** The Rust toolchain is pinned only in `mise.toml` (1.97), matching the workspace; `rust-version` in `Cargo.toml` declares the same minimum. `.githooks/pre-push` re-runs itself under `mise exec` when mise is not active in the calling shell (e.g., `gh repo create --push`, IDEs), so it never falls back to rustup's default toolchain. CI installs it with `jdx/mise-action`, whose mise binary is pinned by version and sha256. Local CLIs (e.g., `dist`, `cargo-deny`) live in `.tools/` (gitignored), installed with `cargo install --root .tools <crate> --locked`.
6. **Rust style.** Every Rust change, in `src/` and `tests/`, must follow [docs/rust-style.md](docs/rust-style.md), an adapted copy of the Google Rust Style Guide. Read it before writing or reviewing Rust. A change that breaks it is not done, even when it compiles and the tests pass.
   - **Checked by tools:** `cargo fmt-style --check` and clippy `-D warnings` run in `.githooks/pre-push` and CI, configured in `.cargo/config.toml`, `[lints.clippy]` in `Cargo.toml`, `clippy.toml`, and `src/lib.rs`. Fix the code instead of weakening them. An accepted exception is `#[expect(clippy::<lint>, reason = "...")]` on the smallest item that needs it, never `#[allow]` and never `--no-verify`.
   - **Not checked by tools:** imports of traits and functions, module layout, `enum` over `bool`, iterator chains, naming, and comments. Passing `make check` says nothing about these, so check each change against the guide by hand.
   - **Conflicts:** the other rules in this file win over the guide.

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
   online rules live in `tests/fixtures/online/`, and each repository rule has a repository in
   `tests/fixtures/repos/<rule>/` that must report it. `tests/rules.rs` checks each `.before.yml` against its
   `# expect: <rule>` markers and fails when a registered rule has no fixture. Markers count:
   `# expect: <rule>, <rule>` requires two findings of that rule on the line. The hand-written
   `.clean.yml` applies the remediation and must not be reported. `.after.yml` is never written by
   hand: it is the exact `--fix` output, regenerated with `CROSGHA_BLESS=1 cargo test --test rules`
   and reviewed in the diff; it is absent when `--fix` leaves the file unchanged.
8. Behavior copied from another tool (zizmor's config, ignore comments, and personas; `migrate`'s view of
   zizmor and actionlint steps) follows that tool's documentation. Link the doc page next to the code, and give
   every case the page describes its own test, not only the first one.

## Layout

```
src/main.rs                 CLI entry point
src/cli.rs                  CLI options, orchestration, exit status
src/workflow.rs             YAML parsing and workflow validation
src/engine.rs               Per-file inspection: parse, run rules, prepare the fix
src/rules/                  Rules by dimension, repository and online rules; shared traversal (common.rs),
                            `${{ }}` expressions (expression.rs), and shell commands (shell.rs)
src/fix.rs                  Shared fix validation
src/filesystem.rs           Workflow discovery and verified writes
src/report.rs               Text, JSON, SARIF, annotation, and agent output; rule catalog
src/policy.rs               Personas, severity, settings, and failure threshold
src/config.rs               crosgha.yml / zizmor.yml discovery and rule settings
src/github.rs               Cached read-only GitHub REST client for online rules
src/migrate.rs              `migrate --from zizmor|actionlint` step rewrites
src/hook.rs                 `hook install|uninstall`: the git pre-push hook
action.yml                  Composite Action metadata and inputs
action/entrypoint.sh        Action: input checks, verified release install, one run, outputs
install.sh                  `curl | sh` installer: verified release binary into ~/.local/bin
docs/ARCHITECTURE.md        Module interfaces and Action flow
docs/rust-style.md          Rust style guide (rule 6)
docs/*.md                   User docs linked from README.md; docs/rules.md is checked against the code by tests/report.rs
docs/rule-examples.md       Generated from tests/fixtures/: `CROSGHA_BLESS=1 cargo test --test report`
tests/cli.rs                end-to-end scans: inputs, discovery, exit status, flags
tests/{fix,config,report,repository,migrate,hook}.rs  end-to-end tests per feature
tests/rules.rs              fixture-driven rule checks, fake GitHub API server
tests/action.rs             Action entry point against a local release server
tests/install.rs            install.sh piped into sh against the same server
tests/common/               shared test helpers: the binary, fixtures, TempDir, the release server
tests/fixtures/             rules/, online/ (+ api.json), repos/, migrate/, and CLI inputs
.githooks/                  commit-msg (Conventional Commits), pre-push (full check)
.github/workflows/          ci.yml, release-please.yml (tags, draft release, dispatches release.yml), release.yml (cargo-dist, workflow_dispatch only)
release-please-config.json  release PR + changelog config (manifest: .release-please-manifest.json)
dist-workspace.toml         cargo-dist target: x86_64-unknown-linux-musl only (static, so it also runs in Alpine job containers)
mise.toml                   Rust version and musl target (the only pin; used locally and in CI)
.cargo/config.toml          `cargo fmt-style` alias: the rustfmt options of rule 6
clippy.toml                 disallowed-methods for rules 1, 2, and 6
deny.toml                   cargo-deny: advisories, licenses, sources, banned detector crates
```

## Hosting
- GitHub repo: `reset-ufc/crosgha`, public (a study comes out of it), default branch `master`, remote `origin`.
- "Allow GitHub Actions to create and approve pull requests" is enabled, so release-please can open its release PR.

No PAT or secret is needed: `release-please.yml` dispatches `release.yml` with the default `GITHUB_TOKEN`.
