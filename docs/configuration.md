# Configuration

## Personas

Rules are grouped by sensitivity, with zizmor's persona names. Each persona includes the ones above it. zizmor's
`--pedantic` is accepted as an alias for `--persona pedantic`.

| Persona | Adds |
|---|---|
| `regular` (default) | High-signal security findings. |
| `pedantic` | Code smells and hardening policies (pinning, timeouts, permissions blocks, triggers). |
| `auditor` | Heuristics that are likely to include false positives (complexity, readability, runtime monitoring). |

## Online rules

Rules with scope `online` in the [rule table](rules.md) query the GitHub API, and so does `repo-org-settings`.

- **When they run:** whenever a token is available (`--gh-token`, `GH_TOKEN`, or `GITHUB_TOKEN`), unless
  `--offline` or `--no-online-audits` is set.
- **GitHub Enterprise Server:** `--gh-hostname` (or `GH_HOST`). It also turns off the checks that describe github.com:
  retired runner images and shut-down action versions in `removed-feature`, and `retired-runtime`.
- **`repo-org-settings`** also needs `--repository owner/name` (or `GITHUB_REPOSITORY`) and a token with the
  Administration (read) repository permission. `GITHUB_TOKEN` cannot have it, so in Actions the rule is usually
  skipped with a warning.
- **Without the API,** `archived-action` still reports the archived actions of the `actions` and `actions-rs`
  organizations from a built-in list.

## Config file

crosgha reads `crosgha.yml` or an existing `zizmor.yml`, found for each input path the same way zizmor finds its
config, or passed with `--config` (also `CROSGHA_CONFIG` or `ZIZMOR_CONFIG`; an empty one counts as unset) for every
input. Discovery starts at the repository root whether the input is the root, its `.github`, `.github/workflows`, or a
workflow in it, and walks up to the Git root; in each directory `.github/crosgha.yml` wins over `crosgha.yml`, then
`.github/zizmor.yml`, then `zizmor.yml` (`.yaml` is accepted too). `--no-config` skips both. Keys may use crosgha's or
zizmor's rule names. Unknown keys, wrong value types, and unknown severities are config errors (exit 2).

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

## Ignoring findings

- **Inline:** `# crosgha: ignore[rule]` anywhere in the finding's YAML span. `# zizmor: ignore[...]` works too, with
  zizmor's rule names.
- **Partial zizmor IDs:** some zizmor IDs name part of a crosgha rule. `unpinned-uses` and `unpinned-images` split
  `unpinned-action`; `bot-conditions` and `unsound-condition` split `unsafe-if-condition`. Ignoring or disabling such
  an ID affects only that part.
- **Override:** `--no-ignores` reports everything, including findings that comments or the config ignore.

## Severity

`--min-severity` hides findings below a threshold on zizmor's scale: `informational` and `low` map to `note`, `medium`
to `warning`, and `high` to `error`. `--fail-on any|note|warning|error|none` (default `any`) sets which findings make
the run fail.
