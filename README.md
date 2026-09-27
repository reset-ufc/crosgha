# crosgha

aCtions Repair Of miSconfigured GitHub Actions: a tool that detects and fixes smells in GitHub Actions workflows.
It covers every tag of the curated smell taxonomy with 45 rules and is built to replace zizmor or actionlint
without rewriting their configuration.

License: TBD by the author.

## Build

```sh
cargo build --release
```

## Run

```sh
./target/release/crosgha                           # scans .github/workflows
./target/release/crosgha path/to/workflow.yml
./target/release/crosgha --persona pedantic .github/workflows
./target/release/crosgha --fix .github/workflows
./target/release/crosgha --list-rules
./target/release/crosgha migrate --from zizmor     # swap zizmor-action for the crosgha Action
```

`PATH` may be one workflow file or a directory; directories are scanned recursively, and symlinked
files are scanned but never rewritten. Exit status: 0 when nothing reaches `--fail-on`, 1 when a
finding does, 2 when a path, workflow, config, write, or API call fails. In that last case no file
is changed.

### Personas

Rules are grouped by sensitivity, with zizmor's persona names:

| Persona | Adds |
|---|---|
| `regular` (default) | High-signal security findings. |
| `pedantic` | Code smells and hardening policies (pinning, timeouts, permissions blocks, triggers). |
| `auditor` | Heuristics that are likely to include false positives (complexity, readability, runtime monitoring). |

### Online rules

Rules marked `online` below query the GitHub API. They run when a token is available (`--gh-token`,
`GH_TOKEN`, or `GITHUB_TOKEN`) unless `--offline` or `--no-online-audits` is set. `--gh-hostname`
(or `GH_HOST`) points them at GitHub Enterprise Server. `repo-org-settings` also needs
`--repository owner/name` (or `GITHUB_REPOSITORY`).

### Ignoring findings

- Inline: `# crosgha: ignore[rule]` anywhere in the finding's YAML span. `# zizmor: ignore[...]`
  works too, with zizmor's rule names.
- Config: `crosgha.yml` or an existing `zizmor.yml`, found the same way zizmor finds its config, or
  passed with `--config` (also `CROSGHA_CONFIG` or `ZIZMOR_CONFIG`). The config supports
  `rules.<id>.disable`, `rules.<id>.ignore` (`file.yml[:line]`), and `rules.<id>.remap.severity`.
- `--no-ignores` reports everything, and `--no-config` skips config discovery.
- `--min-severity` takes zizmor's scale: `informational` and `low` map to `note`, `medium` to
  `warning`, and `high` to `error`.

### Fixes

`--fix` applies a fix only when the patched YAML parses, the targeted finding disappears, no new
finding appears, and only the intended YAML node changed. Today only `persist-credentials` has a
fix. Every other rule is detection-only, because choosing a SHA, timeout, token scope, or trigger
filter needs project context.

### Migrating from zizmor or actionlint

`crosgha migrate --from zizmor|actionlint [PATH]` replaces `zizmorcore/zizmor-action`,
`raven-actions/actionlint`, `reviewdog/action-actionlint`, and `rhysd/actionlint` steps with the
crosgha Action.
- zizmor steps keep their `with:` block, because the crosgha Action accepts zizmor-action's inputs.
- actionlint steps lose theirs.
- Steps that run either tool from a shell are listed so you can change them by hand.

The command writes the latest crosgha release, pinned by SHA with a version comment. Without a
token, pass the reference yourself with `--action owner/repo@<sha> --action-version vX.Y.Z`.

The root `action.yml` is still a scaffold: it validates its inputs, then fails before downloading
a binary. `--format json` and `--format sarif` also fail explicitly. Both are the next steps in
[ROADMAP.md](ROADMAP.md).

## Rules

| Rule | Taxonomy tag | Severity | Persona | Scope | Also answers to |
|---|---|---|---|---|---|
| `unpinned-action` | `unpinned-action` | warning | pedantic | workflow | `unpinned-uses`, `unpinned-images` |
| `unpinned-package` | `unpinned-package` | warning | pedantic | workflow | `adhoc-packages`, `unpinned-tools` |
| `unverified-remote-script` | `unverified-remote-script` | warning | regular | workflow |  |
| `typosquatted-action` | `typosquatting/impostor-commit` | error | regular | workflow | `typosquat-uses` |
| `script-injection` | `script-injection` | error | regular | workflow | `template-injection` |
| `github-env-injection` | `github-env-injection` | error | regular | workflow | `github-env` |
| `llm-prompt-injection` | `llm-prompt-injection` | error | regular | workflow |  |
| `pwn-request` | `pwn-request` | error | regular | workflow |  |
| `dangerous-trigger` | `dangerous-trigger` | warning | pedantic | workflow | `dangerous-triggers` |
| `unsafe-if-condition` | `unsafe-if-condition` | warning | regular | workflow | `bot-conditions`, `unsound-condition`, `if-cond` |
| `artifact-poisoning` | `artifact-poisoning` | warning | regular | workflow |  |
| `cache-poisoning` | `cache-poisoning` | warning | regular | workflow |  |
| `obfuscation` | `obfuscation` | warning | pedantic | workflow |  |
| `excessive-permissions` | `excessive-permissions` | warning | regular | workflow |  |
| `missing-permissions-block` | `missing-permissions-block` | warning | pedantic | workflow |  |
| `secrets-exposure` | `secrets-exposure` | warning | regular | workflow | `overprovisioned-secrets` |
| `secrets-inherit` | `secrets-inherit` | note | pedantic | workflow |  |
| `hardcoded-credentials` | `hardcoded-credentials` | error | regular | workflow | `hardcoded-container-credentials`, `credentials` |
| `persist-credentials` | `persist-credentials` | warning | regular | workflow | `artipacked` |
| `oidc-misconfig` | `oidc-misconfig` | warning | pedantic | workflow |  |
| `self-hosted-runner` | `self-hosted-runner` | warning | regular | workflow |  |
| `runtime-monitoring` | `runtime-monitoring` | note | auditor | workflow |  |
| `runner-versioning` | `runner-versioning` | note | pedantic | workflow |  |
| `missing-timeout` | `missing-timeout` | note | pedantic | workflow |  |
| `error-suppression` | `error-suppression` | note | pedantic | workflow |  |
| `missing-concurrency` | `missing-concurrency` | note | pedantic | workflow | `concurrency-limits` |
| `broad-trigger` | `broad-trigger` | note | pedantic | workflow |  |
| `missing-fork-guard` | `missing-fork-guard` | note | pedantic | workflow |  |
| `missing-matrix` | `missing-matrix/test-diversity` | note | auditor | workflow |  |
| `cache-misuse` | `cache-misuse` | note | pedantic | workflow |  |
| `duplicated-steps` | `redundant/duplicated-steps` | note | pedantic | workflow |  |
| `complex-job` | `long-job/complexity` | note | auditor | workflow |  |
| `readability` | `readability` | note | auditor | workflow | `anonymous-definition` |
| `deprecated-command` | `deprecated-command/feature` | warning | regular | workflow | `insecure-commands`, `deprecated-commands` |
| `schema-validity` | `syntax/schema-validity` | error | regular | workflow | `syntax-check` |
| `embedded-script-lint` | `embedded-script-lint` | note | pedantic | workflow | `shellcheck` |
| `insecure-protocol` | `insecure-protocol` | warning | regular | workflow | `insecure-url-scheme` |
| `known-vulnerable-action` | `known-vulnerable-action` | error | regular | online | `known-vulnerable-actions` |
| `outdated-action` | `outdated-action` | note | pedantic | online |  |
| `version-comment-mismatch` | `version-comment-mismatch` | warning | pedantic | online | `ref-version-mismatch` |
| `impostor-commit` | `typosquatting/impostor-commit` | error | regular | online |  |
| `archived-action` | `untrusted-3rd-party-action` | warning | regular | online | `archived-uses` |
| `missing-release-cooldown` | `missing-release-cooldown` | note | pedantic | repository | `dependabot-cooldown` |
| `missing-security-tooling` | `missing-security-tooling` | note | auditor | repository |  |
| `repo-org-settings` | `repo-org-settings` | warning | pedantic | repository |  |

Rule concepts cite their primary sources in `src/rules/`. The module layout is in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Test

```sh
cargo test
```

## Release

Versioning and releases are automated by release-please (from Conventional Commits) and cargo-dist.

## Development setup

```sh
make setup   # enables the versioned git hooks in .githooks/
make check   # the pre-push checks: fmt, clippy -D warnings, tests, dogfood scan
```
