# our-tool

Rust CLI that will detect smells in GitHub Actions workflows and fix them deterministically, preserving comments and
formatting. Current state: a hello-world binary plus the full release toolchain. There is no detection logic yet.
`our-tool` is a placeholder name; renaming touches only `Cargo.toml` and `README.md`.

This repo is published on its own. When it is checked out inside the research workspace, `CLAUDE.workspace.md` is a
local-only symlink to the workspace guide, which covers the tool map, curated smell taxonomy, and reproduction harness.
It is listed in `.git/info/exclude` and never committed.

## Commands

| Task | Command |
|---|---|
| One-time hook setup after cloning | `make setup` (sets `core.hooksPath=.githooks`) |
| Build / run | `cargo build --release` · `cargo run` |
| Everything the pre-push hook checks | `.githooks/pre-push` (fmt, clippy `-D warnings`, tests) |
| Release plan preview (offline) | `.tools/bin/dist plan` |

## Rules

1. **Independence.** Allowed dependencies are infrastructure crates only, pinned exactly:
   - `github-actions-models`, `yamlpath`, `yamlpatch`, `github-actions-expressions` at `=1.30.1`
   - generic crates such as clap and serde

   Never embed, vendor, or shell out to another detector (zizmor's audit crate, actionlint, poutine, …) for detection or fix validation. Rule *concepts* come from the literature and tool docs, cited in code comments. Our own untrusted-context list is derived from GitHub's docs. No third-party linter runs on this repo either: our own workflows are checked by our-tool itself (dogfood) once it has rules.
2. **Fix validation is ours.** A fix is accepted only when all three hold:
   - the patched file re-parses
   - the targeted smell is gone and no new smell appeared
   - only the intended YAML nodes changed
3. **SemVer from 0.1.0 through Conventional Commits.**
   - The `commit-msg` hook enforces `type(scope)!: subject`.
   - Never bump `version` or edit `CHANGELOG.md` by hand; release-please does both. Before 1.0, breaking changes bump the minor version (`bump-minor-pre-major`). Flow in `RELEASING.md`.
   - Breaking changes use `!` or a `BREAKING CHANGE:` footer.
4. **Dogfood.** Our workflows must stay free of the smells we detect:
   - actions pinned to a 40-char SHA with a `# vX.Y.Z` comment
   - minimal `permissions`
   - `timeout-minutes` on every job
   - `concurrency` in CI
   - `persist-credentials: false`

   Once rules exist, `.githooks/pre-push` and CI run our-tool on `.github/workflows`. Accepted exceptions need a written reason in `RELEASING.md`.
5. **Toolchain.** Rust is pinned in `rust-toolchain.toml` (1.95) and matches the workspace `mise.toml`. Local CLIs (e.g., `dist`) live in `.tools/` (gitignored), installed with `cargo install --root .tools <crate> --locked`.
6. **Tests.** Every rule ships with before/after YAML fixtures and a test asserting the exact fixed output. `tests/cli.rs` runs the real binary end to end.

## Layout

```
src/main.rs                 entry point (hello world for now)
tests/cli.rs                end-to-end test of the built binary
.githooks/                  commit-msg (Conventional Commits), pre-push (full check)
.github/workflows/          ci.yml, release-please.yml (tags, draft release, dispatches release.yml), release.yml (cargo-dist, workflow_dispatch only)
release-please-config.json  release PR + changelog config (manifest: .release-please-manifest.json)
dist-workspace.toml         cargo-dist target: x86_64-unknown-linux-gnu only
ROADMAP.md                  plans: GitHub Action packaging (composite, SHA-verified binary, SARIF)
```

## Going live (not done yet)
1. Create the GitHub repo as **private** (a study comes out of it; the author creates it, not agents) and push `master`.
2. Replace the `repository` placeholder in `Cargo.toml`.
3. Enable "Allow GitHub Actions to create and approve pull requests".

No PAT or secret is needed: `release-please.yml` dispatches `release.yml` with the default `GITHUB_TOKEN`.

Details in `RELEASING.md`.
