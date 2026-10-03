# Architecture

`crosgha` has one analysis path, shared by the CLI, the pre-push hook, and the GitHub Action.
The hook and the Action both run the CLI binary; neither contains a second detector or fixer.

| Area | Current state |
|---|---|
| CLI pipeline | Runnable: personas, `fail-on`, zizmor-compatible flags, and `migrate`. |
| Rule modules | 48 rules covering every taxonomy tag: 39 per workflow, 4 per repository, 5 online. |
| Config and ignores | `crosgha.yml` or `zizmor.yml` (disable, ignore, remap, and per-rule options), and inline `crosgha:` or `zizmor:` ignore comments. |
| Fix validation | Runnable for add and replace proposals; `persist-credentials` and `removed-feature` propose fixes. |
| Reports | Text, versioned JSON, SARIF 2.1.0, GitHub annotations (default under `GITHUB_ACTIONS=true`), and brief agent lines from the same findings; `--sarif-file` adds SARIF to any format. |
| Composite Action | Report mode works: zizmor-action's inputs, a sha256-verified release binary, SARIF upload, job summary, then `fail-on`. |
| PR and auto-merge | Mode contract and guards exist; repository operations are pending. |

## Execution path

```text
CLI (directly, from the pre-push hook, or from the Action)
    -> discover workflow files and the config
    -> parse and validate each workflow
    -> run the rules enabled by persona and config (online rules only with a token)
    -> drop ignored findings, remap severities, apply --min-severity
    -> validate every proposed patch
    -> run repository rules for the .github directory next to the scanned workflows
    -> write verified files when --fix is requested and every workflow parsed
    -> render the report and choose an exit status
```

The stages have these interfaces:

| Module | Input | Output and responsibility |
|---|---|---|
| `filesystem` | File or directory path | Sorted `.yml`/`.yaml` paths, file contents, the owning `.github` directory, and per-file verified writes. |
| `workflow` | YAML source | A `yamlpath::Document` and semantic YAML value; `parse` also validates the `on`/`jobs` structure. A parse error is reported for that file, the other files are still checked, and the run exits with status 2 without writing. |
| `config` | Scanned path or `--config` | Per-rule `disable`, `ignore`, severity remaps, and `config` options, using zizmor's schema and discovery order. |
| `rules` | Parsed workflow and settings | Findings with rule ID, curated tag, severity, location, route, and an optional fix proposal. Rules do not write files. |
| `rules::common` | Parsed workflow | Shared traversal: jobs, steps (including `parallel` groups), triggers, expressions, untrusted contexts, and `uses` refs. |
| `github` | API path | Cached read-only GitHub REST responses for online rules, failures included. Each path has its own lock, so parallel workflows request it once; after a rate-limit refusal no further request is sent. |
| `engine` | Source and scan/fix mode | Findings and a prepared fix result. This is the main in-memory analysis interface. Workflows and `.github` directories are inspected on every core; results keep path order, and fixes are written only after all inspections finish. |
| `fix` | Parsed workflow and proposals | Reparsed source after each patch, proof that the target disappeared, no new finding appeared, and only intended YAML nodes changed. A rejected proposal keeps its finding, records the reason, and the next proposal still runs. |
| `report` | Per-file and repository findings | Text, JSON, SARIF, annotation, and agent renderings of the same findings, plus the rule catalog as text or JSON. |
| `policy` | Persona, severity, and `fail-on` | Which rules run, and whether reported findings make the command fail. |
| `migrate` | Workflows and the tool to replace | Rewrites zizmor or actionlint Action steps and install/run shell steps to the crosgha Action, and writes `crosgha.yml` from the tool's config. Every file is rewritten in memory and re-parsed before any is written. |
| `hook` | Current git repository | Writes or removes the `pre-push` hook that runs the CLI; never touches a hook it did not write. |
| `cli` | Command-line options | Preflight, output, exit policy, and the decision to persist verified fixes. |

`src/main.rs` only enters the CLI. The library modules hold the behavior so
additional entry points can reuse the same analysis path.

## Rule contract

Each rule lives in `src/rules/` and declares:
- `id`: stable, and the name used in ignores and the config.
- `tag`: one from the curated smell taxonomy (`TAXONOMY` in the workspace's `build_matrix.py`).
- `summary`: one sentence, used as SARIF's descriptions and help.
- `severity`: a SARIF level.
- `persona`: the lowest persona that runs it.
- `equivalents`: `(tool, id)` pairs for the same concept in zizmor or actionlint.
- `configurable`: whether it reads `rules.<id>.config`.

Equivalent IDs keep existing ignore comments and `zizmor.yml` entries working after a switch, and
the report names them. The concept and primary source are cited above each rule. Rules come in
four shapes, dispatched from explicit tables in `rules/mod.rs`:

| Shape | Signature | Registered in |
|---|---|---|
| Workflow | `fn(&Workflow) -> Vec<Finding>` | `REGISTRY` |
| Configurable | `fn(&Workflow, Options) -> Vec<Finding>` with its `rules.<id>.config` | `CONFIGURABLE` |
| Online | `fn(&Workflow, &GitHub, Options) -> Vec<Finding>` | `ONLINE`; runs only with a token |
| Repository | `fn(&Path, &Scanned, bool, Options) -> Vec<Located>` for the `.github` directory; `Scanned` holds the workflows the scan already parsed, so they are not read twice | `REPOSITORY`; `repo-org-settings` also needs `--repository` |

Checks that state github.com facts (retired hosted-runner images and service shutdowns in
`removed-feature`, and `retired-runtime`) sit in the `HOSTED` and `HOSTED_REPOSITORY` tables and are
skipped when `--gh-hostname` names another host.

A finding can have no fix when the change is not safe to choose automatically. A fix proposal
supplies a `yamlpatch` operation, the route it may change, and whether it adds or replaces that
node. The shared fix module applies and validates proposals. Detection-only rules leave source
unchanged under `--fix` and remain visible in the report.

## Fix safety

1. Parse the patched YAML and validate its workflow structure.
2. Run every enabled rule again. The target must disappear; no new
   finding may appear. `duplicated-steps` compares steps with each other, so fixing one of two
   equal steps may clear its finding, and fixing the other may bring back one the file had.
3. Check that only the declared node changed, byte for byte, comments and spacing included.
   A replaced value is swapped back and must restore the original source. An added key must
   be the only insertion: the original equals the patched source minus the new node and the
   spaces, line breaks, or comma around it, and the parsed values differ only by that key.
   Files with CRLF or mixed line breaks, or without a final newline, are patched as LF with
   one; then each original line gets its own break back and an added line takes the break of
   the line before it.
4. Prepare all files before writing any file. Write each changed file through
   a same-directory temporary file, preserving permissions and renaming it
   over the original. A symbolic link is never replaced, nor a file whose content changed
   since it was read (an edit saved during the scan). A failed write is
   reported, exits with status 2, and leaves the other files fixed; there is no
   cross-file transaction.

This validation applies to every new rule's fix. A rule that cannot describe
and verify its intended node changes remains detection-only.

## Reports and exit policy

Text (`plain` in zizmor's terms), JSON, SARIF, GitHub annotations, and agent lines render the same findings; no format runs its
own detection. JSON is versioned (`version: 1`) with a `summary` and one object per finding,
including its span and equivalents. Under `--fix`, `fixes` lists every proposed fix with its
status: `applied`, `rejected` with the validation reason, or `not-written` with the reason the
file kept its original content. Every format reports what is on disk: the findings that remain
in a written file, and the original findings of a file whose fixes were not written. SARIF follows GitHub's
[SARIF support](https://docs.github.com/en/code-security/reference/code-scanning/sarif-files/sarif-support):
- every rule has descriptions, help, a default level, and a precision derived from its persona;
- security rules carry `security-severity`, and maintainability and correctness rules carry
  `problem.severity`;
- every result in a file has a percent-encoded relative URI and a full region whose columns
  count UTF-16 code units (`columnKind`); a repository-wide finding names its directory in the
  message and points at line 1 of the first scanned workflow of that directory, because code
  scanning shows no result without a location;
- `partialFingerprints` is left to `upload-sarif`.

zizmor's severities map onto the SARIF levels: `informational`/`low` → `note`, `medium` →
`warning`, `high` → `error`.
`any` is the default `--fail-on`, and `none` disables failure on findings. `--fix` evaluates
the findings that remain after fixes.

Input, parse, config, API, and write errors exit with status 2. A workflow that fails to parse is
reported on stderr while the others are still checked and reported; `--fix` then writes nothing. JSON lists
such files, files whose fix could not be written, and API failures that skipped online checks under
`errors`; SARIF lists them as `toolExecutionNotifications` with `executionSuccessful: false`, and the
text summary drops its ✓ and says when the scan is incomplete. Exit codes
deliberately stay 0/1/2 instead of zizmor's 11–14. CI steps usually check only for non-zero.

A GitHub API failure (rate limit, server error, network, rejected token) skips only the online checks
that needed that answer; every other finding is reported in every format, and each failed request is
named once on stderr. The scan is incomplete, so it exits 2 and `--fix` writes nothing: a CI gate must
not pass on checks that never ran. `repo-org-settings` is the exception: reading Actions settings needs
the Administration (read) permission, which `GITHUB_TOKEN` cannot be granted, so a 403 or 404 there
skips that rule with a warning and leaves the exit status to the findings.

## GitHub Action path

The composite Action (`action.yml`, `action/entrypoint.sh`) is a distribution layer
over this CLI. Its inputs are a superset of `zizmorcore/zizmor-action`'s, so
`crosgha migrate --from zizmor` swaps only the `uses:` line. Its sequence is:

1. Validate Linux runner, persona, severities, modes, token requirements, and
   incompatible inputs.
2. Resolve the release version associated with the action revision. Download
   the matching Linux asset and verify it against the release `sha256.sum` before execution.
3. Run the CLI once and produce findings, job summary, and SARIF output.
4. With `advanced-security: true`, upload SARIF using a SHA-pinned upload
   action; the caller grants `security-events: write`.
5. In requested PR mode, run verified fixes, then create a branch and a PR with
   the supplied `fix-token` (GitHub App token or fine-grained PAT) that lists
   each rule and fix. Auto-merge is a separate explicit opt-in and requires the
   checks described in [ROADMAP.md](../ROADMAP.md). The Action never merges directly.

Steps 1-4 are implemented in `action/entrypoint.sh` and `action.yml`. The release version comes
from the Action's own `Cargo.toml`, which release-please bumps in the tagged commit, and tags are
`vX.Y.Z` (`include-component-in-tag: false`). The entry point never fails on findings: it records
`exit-code`, the upload step runs, and a last step applies it. Step 5 (PR and auto-merge modes) is
validated and then refused until its repository operations have end-to-end fixtures.

## Verification surface

| Test file | What it runs |
|---|---|
| `tests/rules.rs` | Scans every `tests/fixtures/rules/<rule>[.<variant>].before.yml` at the `auditor` persona. Findings of the rule must match the `# expect: <rule>` markers exactly, `--fix` must remove all or none of them and write exactly `.after.yml` (or nothing when that file is absent; `CROSGHA_BLESS=1` regenerates it), and the hand-written `.clean.yml` must not be reported. |
| `tests/rules.rs` (online) | Runs the online fixtures against a local server that replays `tests/fixtures/online/api.json` and rejects wrong tokens. |
| `tests/rules.rs` (coverage) | Fails if any registered rule lacks a fixture. |
| `tests/cli.rs` | The binary end to end: fixes, exit codes, symlinks, ignores, config discovery, repository rules, report formats, `migrate`, and `hook`. |
| `tests/action.rs` | `action/entrypoint.sh` with a runner environment against a local release server: verified install, SARIF and outputs, a tampered checksum, and `fail-on-no-inputs`. |

The pre-push hook and CI run formatting, Clippy, the tests, and crosgha itself on
`.github/workflows` at the `pedantic` persona in offline mode.
