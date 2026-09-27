# crosgha

aCtions Repair Of miSconfigured GitHub Actions: a tool that detects and fixes smells in GitHub Actions workflows.
It covers every tag of the curated smell taxonomy with 46 rules and is built to replace zizmor or actionlint
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
./target/release/crosgha --format sarif > results.sarif   # also: json, github
./target/release/crosgha --sarif-file results.sarif .      # text on stdout, SARIF on disk
./target/release/crosgha --list-rules
./target/release/crosgha migrate --from zizmor     # swap zizmor-action for the crosgha Action
```

Each `PATH` may be a workflow file, a directory (scanned recursively), or a repository root, which
is scanned through its `.github/workflows` as zizmor does with `.`. Symlinked files are scanned but
never rewritten. Exit status: 0 when nothing reaches `--fail-on`, 1 when a
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
  `rules.<id>.disable`, `rules.<id>.ignore` (`file.yml[:line]`), `rules.<id>.remap.severity`, and
  the `rules.<id>.config` options below. Keys may use crosgha's or zizmor's rule names. A
  `config` block that no crosgha rule reads produces a warning.

  | Option | Rule |
  |---|---|
  | `unpinned-uses.config.policies` (`pattern: hash-pin \| ref-pin \| any`) | `unpinned-action` |
  | `forbidden-uses.config.allow` or `.deny` (repository patterns) | `forbidden-action` |
  | `known-vulnerable-actions.config.allow` (advisory IDs) | `known-vulnerable-action` |
  | `dependabot-cooldown.config.days` (default 7) | `missing-release-cooldown` |
  | `self-hosted-runner.config.labels` (custom runner labels, `*` wildcards) | `self-hosted-runner` |
- Some zizmor IDs name part of a crosgha rule: `unpinned-uses` and `unpinned-images` split
  `unpinned-action`, and `bot-conditions` and `unsound-condition` split `unsafe-if-condition`.
  Ignoring or disabling such an ID affects only that part.
- `--no-ignores` reports everything, and `--no-config` skips config discovery.
- `--min-severity` takes zizmor's scale: `informational` and `low` map to `note`, `medium` to
  `warning`, and `high` to `error`.

### Fixes

`--fix` applies a fix only when the patched YAML parses, the targeted finding disappears, no new
finding appears, and only the intended YAML node changed. Today only `persist-credentials` has a
fix. Every other rule is detection-only, because choosing a SHA, timeout, token scope, or trigger
filter needs project context.

### Output

Text lines end with the equivalent rule in other tools, such as `(zizmor: template-injection;
actionlint: expression)`. `--format json` writes a versioned report (`version`, `summary`,
`findings`). `--format sarif` writes SARIF 2.1.0 with the rule metadata GitHub code scanning
requires; it validates against the official schema. `--format github` prints workflow-command
annotations. `--sarif-file FILE` also writes SARIF during any format, so one run serves the log and
the Security tab. After `--fix`, every format lists what remains.

### GitHub Action

```yaml
permissions:
  contents: read
  security-events: write # only for the SARIF upload (advanced-security: true, the default)
steps:
  - uses: actions/checkout@<sha> # vX.Y.Z
    with:
      persist-credentials: false
  - uses: meiazero/crosgha@<sha> # vX.Y.Z
    with:
      persona: pedantic
```

The Action takes zizmor-action's inputs (`inputs`, `persona`, `min-severity`, `online-audits`,
`token`, `advanced-security`, `annotations`, `config`, `fail-on-no-inputs`, and more) plus
`fail-on`. It works like this:
1. It downloads the release that matches its own revision and checks the archive against the
   release's `.sha256` before running it.
2. It runs crosgha once. The log and the job summary get the text report.
3. It uploads SARIF with `github/codeql-action/upload-sarif`, pinned by SHA.
4. Only then does it fail according to `fail-on`.

Linux runners only. `fix: pull-request` is validated but not implemented yet.

### Migrating from zizmor or actionlint

`crosgha migrate --from zizmor|actionlint [PATH]` moves a repository over:
- **Action steps:** `zizmorcore/zizmor-action`, `raven-actions/actionlint`, `reviewdog/action-actionlint`,
  and `rhysd/actionlint` steps become crosgha Action steps. zizmor steps keep their `with:` block
  (the crosgha Action accepts zizmor-action's inputs); actionlint steps lose theirs.
- **Shell steps:** steps that only install the tool are removed. Steps that only run it become
  crosgha Action steps, with CLI flags translated to inputs. Steps that mix the tool with other
  commands are listed for a manual change.
- **Config:** `zizmor.yml` becomes `crosgha.yml` next to it, with rule keys renamed where the match
  is one to one and keys without a crosgha rule dropped. `actionlint.yaml` self-hosted runner labels
  become `rules.self-hosted-runner.config.labels`. The original config stays in place.

The command writes the latest crosgha release, pinned by SHA with a version comment. Without a
token, pass the reference yourself with `--action owner/repo@<sha> --action-version vX.Y.Z`.

## Rules

| Rule | Taxonomy tag | Severity | Persona | Scope | Equivalent in | Config |
|---|---|---|---|---|---|---|
| `unpinned-package` | `unpinned-package` | warning | pedantic | workflow | zizmor: `adhoc-packages`, `unpinned-tools` |  |
| `unverified-remote-script` | `unverified-remote-script` | warning | regular | workflow |  |  |
| `typosquatted-action` | `typosquatting/impostor-commit` | error | regular | workflow | zizmor: `typosquat-uses` |  |
| `script-injection` | `script-injection` | error | regular | workflow | zizmor: `template-injection`; actionlint: `expression` |  |
| `github-env-injection` | `github-env-injection` | error | regular | workflow | zizmor: `github-env` |  |
| `llm-prompt-injection` | `llm-prompt-injection` | error | regular | workflow |  |  |
| `pwn-request` | `pwn-request` | error | regular | workflow |  |  |
| `dangerous-trigger` | `dangerous-trigger` | warning | pedantic | workflow | zizmor: `dangerous-triggers` |  |
| `unsafe-if-condition` | `unsafe-if-condition` | warning | regular | workflow | zizmor: `bot-conditions`, `unsound-condition`; actionlint: `if-cond` |  |
| `artifact-poisoning` | `artifact-poisoning` | warning | regular | workflow |  |  |
| `cache-poisoning` | `cache-poisoning` | warning | regular | workflow | zizmor: `cache-poisoning` |  |
| `obfuscation` | `obfuscation` | warning | pedantic | workflow | zizmor: `obfuscation` |  |
| `excessive-permissions` | `excessive-permissions` | warning | regular | workflow | zizmor: `excessive-permissions` |  |
| `missing-permissions-block` | `missing-permissions-block` | warning | pedantic | workflow |  |  |
| `secrets-exposure` | `secrets-exposure` | warning | regular | workflow | zizmor: `overprovisioned-secrets` |  |
| `secrets-inherit` | `secrets-inherit` | note | pedantic | workflow | zizmor: `secrets-inherit` |  |
| `hardcoded-credentials` | `hardcoded-credentials` | error | regular | workflow | zizmor: `hardcoded-container-credentials`; actionlint: `credentials` |  |
| `persist-credentials` | `persist-credentials` | warning | regular | workflow | zizmor: `artipacked` |  |
| `oidc-misconfig` | `oidc-misconfig` | warning | pedantic | workflow |  |  |
| `runtime-monitoring` | `runtime-monitoring` | note | auditor | workflow |  |  |
| `runner-versioning` | `runner-versioning` | note | pedantic | workflow |  |  |
| `missing-timeout` | `missing-timeout` | note | pedantic | workflow |  |  |
| `error-suppression` | `error-suppression` | note | pedantic | workflow |  |  |
| `missing-concurrency` | `missing-concurrency` | note | pedantic | workflow | zizmor: `concurrency-limits` |  |
| `broad-trigger` | `broad-trigger` | note | pedantic | workflow |  |  |
| `missing-fork-guard` | `missing-fork-guard` | note | pedantic | workflow |  |  |
| `missing-matrix` | `missing-matrix/test-diversity` | note | auditor | workflow |  |  |
| `cache-misuse` | `cache-misuse` | note | pedantic | workflow |  |  |
| `duplicated-steps` | `redundant/duplicated-steps` | note | pedantic | workflow |  |  |
| `complex-job` | `long-job/complexity` | note | auditor | workflow |  |  |
| `readability` | `readability` | note | auditor | workflow | zizmor: `anonymous-definition` |  |
| `deprecated-command` | `deprecated-command/feature` | warning | regular | workflow | zizmor: `insecure-commands`; actionlint: `deprecated-commands` |  |
| `schema-validity` | `syntax/schema-validity` | error | regular | workflow | actionlint: `syntax-check` |  |
| `embedded-script-lint` | `embedded-script-lint` | note | pedantic | workflow | actionlint: `shellcheck` |  |
| `insecure-protocol` | `insecure-protocol` | warning | regular | workflow | zizmor: `insecure-url-scheme` |  |
| `unpinned-action` | `unpinned-action` | warning | pedantic | workflow | zizmor: `unpinned-uses`, `unpinned-images` | yes |
| `forbidden-action` | `untrusted-3rd-party-action` | warning | regular | workflow | zizmor: `forbidden-uses` | yes |
| `self-hosted-runner` | `self-hosted-runner` | warning | regular | workflow | zizmor: `self-hosted-runner` | yes |
| `known-vulnerable-action` | `known-vulnerable-action` | error | regular | online | zizmor: `known-vulnerable-actions` | yes |
| `outdated-action` | `outdated-action` | note | pedantic | online |  |  |
| `version-comment-mismatch` | `version-comment-mismatch` | warning | pedantic | online | zizmor: `ref-version-mismatch` |  |
| `impostor-commit` | `typosquatting/impostor-commit` | error | regular | online | zizmor: `impostor-commit` |  |
| `archived-action` | `untrusted-3rd-party-action` | warning | regular | online | zizmor: `archived-uses` |  |
| `missing-release-cooldown` | `missing-release-cooldown` | note | pedantic | repository | zizmor: `dependabot-cooldown` | yes |
| `missing-security-tooling` | `missing-security-tooling` | note | auditor | repository |  |  |
| `repo-org-settings` | `repo-org-settings` | warning | pedantic | repository |  |  |

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
