# crosgha

aCtions Repair Of miSconfigured GitHub Actions: a Rust CLI and GitHub Action that detects smells in GitHub Actions
workflows and fixes them deterministically, preserving comments and formatting.

- 48 rules cover every tag of a curated smell taxonomy: 39 check workflow files, 4 check the repository, and 5 query
  the GitHub API.
- It reads `zizmor.yml`, accepts zizmor's rule names and ignore comments, and names the equivalent zizmor or
  actionlint rule in every finding, so switching tools does not mean rewriting configuration.
- Output is text, JSON, SARIF 2.1.0, GitHub annotations (default under `GITHUB_ACTIONS=true`), or brief lines for AI
  agents.

License: TBD by the author.

**Contents:** [Install](#install) · [Quick start](#quick-start) · [GitHub Action](#github-action) ·
[Pre-push hook](#pre-push-hook) · [Migrating from zizmor or actionlint](#migrating-from-zizmor-or-actionlint) ·
[Configuration](#configuration) · [Output and exit status](#output-and-exit-status) · [Fixes](#fixes) ·
[Rules](#rules) · [Development](#development)

## Install

### Install script

On Linux x86_64, this installs the latest release to `~/.local/bin` after checking it against the release's
`sha256.sum`:

```sh
curl -fsSL https://crosgha.meiazero.dev/install.sh | sh
```

`CROSGHA_VERSION=0.9.0` pins a release and `CROSGHA_INSTALL_DIR` changes the target directory. Uninstall with
`rm ~/.local/bin/crosgha`.

### From a release

Releases ship one Linux binary (`x86_64-unknown-linux-gnu`) with a `sha256.sum`:

```sh
gh release download -R meiazero/crosgha -p crosgha-x86_64-unknown-linux-gnu.tar.xz -p sha256.sum
sha256sum -c sha256.sum
tar -xJf crosgha-x86_64-unknown-linux-gnu.tar.xz
install crosgha-x86_64-unknown-linux-gnu/crosgha ~/.local/bin/
```

### From source

Needs Rust 1.97, pinned in `mise.toml`:

```sh
mise install
cargo build --release   # binary at target/release/crosgha
```

## Quick start

```sh
crosgha                                   # scans .github/workflows
crosgha path/to/workflow.yml
crosgha --persona pedantic .github/workflows
crosgha --fix .github/workflows
crosgha -f sarif > results.sarif          # also: json, github, agent
crosgha --sarif-file results.sarif .      # text on stdout, SARIF on disk
crosgha --list-rules                      # add -f json for summaries and equivalents
```

Each `PATH` may be a workflow file, a directory (scanned recursively), a repository root, or its `.github`; the last
two are scanned through `.github/workflows` as zizmor does with `.`. Without a `PATH`, crosgha scans
`.github/workflows`. A `dependabot.yml` path gets only the Dependabot cooldown check. Symlinked files are scanned but
never rewritten.

## GitHub Action

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

Pin the Action by full commit SHA with a version comment; there is no moving `v1` tag. It takes zizmor-action's inputs
(`inputs`, `persona`, `min-severity`, `online-audits`, `token`, `advanced-security`, `annotations`, `config`,
`fail-on-no-inputs`, and more) plus `fail-on`, and sets the outputs `output-file` (SARIF path) and `findings` (count).
A run goes like this:

1. It downloads the release that matches its own revision and checks the archive against the release's `sha256.sum`
   before running it.
2. It runs crosgha once. The log and the job summary get the text report.
3. It uploads SARIF with `github/codeql-action/upload-sarif`, pinned by SHA.
4. Only then does it fail according to `fail-on`.

Linux runners only. `fix: pull-request` is validated but not implemented yet; see [ROADMAP.md](ROADMAP.md).

## Pre-push hook

```sh
crosgha hook install -- --offline --persona pedantic
crosgha hook uninstall
```

`install` writes a git `pre-push` hook in the directory git resolves for hooks, so `core.hooksPath` is respected.

- **Arguments:** everything after `--` is validated and passed to every scan. Re-running `install` replaces them.
- **What it runs:** `crosgha` from `PATH`, at the repository root, on the working tree (uncommitted edits count).
- **On findings:** it blocks the push and tells you to run `crosgha --fix`. It never edits files itself.
- **Safety:** both commands refuse to touch a `pre-push` hook that crosgha did not write.

## Migrating from zizmor or actionlint

`crosgha migrate --from zizmor|actionlint [PATH]` moves a repository over:

- **Action steps:** `zizmorcore/zizmor-action`, `raven-actions/actionlint`, `reviewdog/action-actionlint`, and
  `rhysd/actionlint` steps become crosgha Action steps. zizmor steps keep their `with:` block (the crosgha Action
  accepts zizmor-action's inputs); actionlint steps lose theirs.
- **Shell steps:** steps that only install the tool are removed. Steps that only run it become crosgha Action steps,
  with CLI flags translated to inputs. Steps that mix the tool with other commands are listed for a manual change.
- **Config:** `zizmor.yml` becomes `crosgha.yml` next to it, with rule keys renamed where the match is one to one and
  keys without a crosgha rule dropped. `actionlint.yaml` self-hosted runner labels become
  `rules.self-hosted-runner.config.labels`. The original config stays in place.

The command writes the latest crosgha release, pinned by SHA with a version comment. Without a token, pass the
reference yourself with `--action owner/repo@<sha> --action-version vX.Y.Z`.

## Configuration

### Personas

Rules are grouped by sensitivity, with zizmor's persona names. Each persona includes the ones above it.

| Persona | Adds |
|---|---|
| `regular` (default) | High-signal security findings. |
| `pedantic` | Code smells and hardening policies (pinning, timeouts, permissions blocks, triggers). |
| `auditor` | Heuristics that are likely to include false positives (complexity, readability, runtime monitoring). |

### Online rules

Rules with scope `online` in the [rule table](#rules) query the GitHub API.

- **When they run:** whenever a token is available (`--gh-token`, `GH_TOKEN`, or `GITHUB_TOKEN`), unless
  `--offline` or `--no-online-audits` is set.
- **GitHub Enterprise Server:** `--gh-hostname` (or `GH_HOST`). It also turns off the checks that describe github.com:
  retired runner images and shut-down action versions in `removed-feature`, and `retired-runtime`.
- **`repo-org-settings`** also needs `--repository owner/name` (or `GITHUB_REPOSITORY`).

### Config file

crosgha reads `crosgha.yml` or an existing `zizmor.yml`, found the same way zizmor finds its config, or passed with
`--config` (also `CROSGHA_CONFIG` or `ZIZMOR_CONFIG`). `--no-config` skips discovery. Keys may use crosgha's or
zizmor's rule names.

| Key | Effect |
|---|---|
| `rules.<id>.disable` | Turns the rule off. |
| `rules.<id>.ignore` | Ignores findings in `file.yml` or at `file.yml:line`. |
| `rules.<id>.remap.severity` | Changes the rule's severity. |
| `rules.<id>.config` | Rule options, listed below. A `config` block that no crosgha rule reads produces a warning. |

| Option | Rule |
|---|---|
| `unpinned-uses.config.policies` (`pattern: hash-pin \| ref-pin \| any`) | `unpinned-action` |
| `forbidden-uses.config.allow` or `.deny` (repository patterns) | `forbidden-action` |
| `known-vulnerable-actions.config.allow` (advisory IDs) | `known-vulnerable-action` |
| `dependabot-cooldown.config.days` (default 7) | `missing-release-cooldown` |
| `self-hosted-runner.config.labels` (custom runner labels, `*` wildcards) | `self-hosted-runner` |

### Ignoring findings

- **Inline:** `# crosgha: ignore[rule]` anywhere in the finding's YAML span. `# zizmor: ignore[...]` works too, with
  zizmor's rule names.
- **Partial zizmor IDs:** some zizmor IDs name part of a crosgha rule. `unpinned-uses` and `unpinned-images` split
  `unpinned-action`; `bot-conditions` and `unsound-condition` split `unsafe-if-condition`. Ignoring or disabling such
  an ID affects only that part.
- **Override:** `--no-ignores` reports everything, including findings that comments or the config ignore.

### Severity

`--min-severity` hides findings below a threshold on zizmor's scale: `informational` and `low` map to `note`, `medium`
to `warning`, and `high` to `error`. `--fail-on any|note|warning|error|none` (default `any`) sets which findings make
the run fail.

## Output and exit status

| `--format` | Output |
|---|---|
| `text` (default; `plain` in zizmor) | One rustc-style diagnostic per finding: the source line with the node underlined, the rule's help, and the equivalent rule in other tools, such as `same as zizmor: template-injection; actionlint: expression`. A summary counts findings by severity. |
| `json` | Versioned report: `version`, `summary`, `findings`, and `errors` for workflows that failed to parse. |
| `sarif` | SARIF 2.1.0 with the rule metadata GitHub code scanning requires; it validates against the official schema. Parse failures are `toolExecutionNotifications` of the invocation. |
| `github` | Workflow-command annotations. The default when `GITHUB_ACTIONS=true`. |
| `agent` | One brief line per finding for AI agents: `path:line:col: severity rule: message help: <rule summary>`, after oxlint's format of the same name. |

`--sarif-file FILE` also writes SARIF during any format, so one run serves the log and the Security tab. After `--fix`,
every format lists what remains. Text is colored on a terminal; `--color always|never` overrides that, and `NO_COLOR`
turns it off.

| Exit status | Meaning |
|---|---|
| 0 | No finding reaches `--fail-on`. |
| 1 | A finding does. |
| 2 | A path, workflow, config, write, or API call failed. No file is changed; findings in the workflows that did parse are still reported, and JSON and SARIF list the failed files. |

## Fixes

`--fix` applies a fix only when all of these hold:

1. the patched YAML parses;
2. the targeted finding disappears and no new finding appears;
3. only the intended YAML node changed.

Two rules have fixes:

- `persist-credentials` sets `persist-credentials: false`.
- `removed-feature` replaces the AdoptOpenJDK distributions that setup-java v6 removed with the successor its README
  names.

Every other rule is detection-only, because choosing a SHA, timeout, token scope, or trigger filter needs project
context.

## Rules

Rule concepts cite their primary sources in `src/rules/`. `cargo run -- --list-rules` prints this catalog from the
code. **Config** marks rules that read `rules.<id>.config`.

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
| `removed-feature` | `deprecated-command/feature` | error | regular | workflow |  |  |
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
| `retired-runtime` | `deprecated-command/feature` | warning | regular | repository |  |  |
| `repo-org-settings` | `repo-org-settings` | warning | pedantic | repository |  |  |

## Development

```sh
make setup   # enables the versioned git hooks in .githooks/
cargo test   # end-to-end and fixture-driven rule tests
make check   # the pre-push checks: fmt, clippy -D warnings, tests, dogfood scan
```

- **Toolchain:** the pre-push hook runs under `mise exec` when mise is not active in the calling shell, so pushes from
  `gh` or an IDE use the pinned toolchain too.
- **Releases:** commits follow Conventional Commits (enforced by the `commit-msg` hook). release-please turns them
  into versions and the changelog, and cargo-dist builds the release binaries.
- **Internals:** module interfaces, the rule contract, and fix safety are in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
