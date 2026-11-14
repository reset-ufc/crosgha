# Architecture

`crosgha` has one analysis path, shared by the CLI, the pre-push hook, and the GitHub Action.
The hook and the Action both run the CLI binary; neither contains a second detector or fixer.

| Area | Current state |
|---|---|
| CLI pipeline | Runnable: personas, `fail-on`, zizmor-compatible flags, and `migrate`. |
| Rule modules | 51 rules covering every taxonomy tag: 43 per file, 3 per repository, 5 online. Step rules also run on composite actions' `runs.steps`, and metadata checks on action files, local or read from another repository with `--follow-actions`. |
| Config and ignores | `crosgha.yml` or `zizmor.yml` (disable, ignore, remap, and per-rule options), and inline `crosgha:` or `zizmor:` ignore comments. |
| Fix validation | Runnable for add, replace, and remove proposals; `persist-credentials` and `error-suppression` propose adds of a step input, `removed-feature` replacements, `deprecated-command` and `embedded-script-lint` line rewrites inside a `run` script, `unsafe-if-condition` the new source of an `if` scalar, `readability` the new source of a step value, and `redundant-setting` removals of a key. |
| Reports | Text, versioned JSON, SARIF 2.1.0, GitHub annotations (default under `GITHUB_ACTIONS=true`), and brief agent lines from the same findings; `--sarif-file` adds SARIF to any format. |
| Composite Action | Report mode works: zizmor-action's inputs, a sha256-verified release binary, SARIF upload, job summary, then `fail-on`. |
| PR and auto-merge | PR mode works: `crosgha --fix`, a commit of only the fixed files to `crosgha/fix-<run_id>`, and a PR opened with `fix-token`. Auto-merge enables GitHub auto-merge on that PR when the base branch requires status checks. |

## Execution path

```text
CLI (directly, from the pre-push hook, or from the Action)
    -> discover workflow and action files and the config (a remote input: resolve the ref to a commit and list its tree)
    -> parse and validate each file as a workflow or an action
    -> run the rules enabled by persona and config that read that kind of file (online rules only with a token)
    -> add each repository's root action and the local actions its files call that resolve inside it, until none is new
    -> with --follow-actions, read the actions those files call from other repositories at their commit and check them,
       following composite actions breadth first (each owner/repo/path@sha once, at most 9 levels deep)
    -> drop ignored findings, remap severities, apply --min-severity
    -> validate every proposed patch
    -> run repository rules for the repository around the scanned workflows
    -> write verified files when --fix is requested and every workflow parsed (never for remote inputs)
    -> render the report and choose an exit status
```

A local directory and a remote repository go through the same steps. What differs is behind `tree::Tree`, the
repository's files: `LocalTree` reads a directory, and `RemoteTree` reads one commit through the GitHub API. Action
discovery (`tree::actions`), the repository rules, and remote config discovery take a `&dyn Tree`, so each has one
body for both kinds of input.

The stages have these interfaces:

| Module | Input | Output and responsibility |
|---|---|---|
| `filesystem` | File or directory path | Sorted `.yml`/`.yaml` paths, whether each is a workflow or an action (`kind`: `action.yml`/`action.yaml` outside `.github/workflows`), the metadata file of an action directory (`action.yml` before `action.yaml`, as the runner reads them), the repository root and `.github` directory of an input, file contents without a BOM (`Source`), and per-file verified writes that put the BOM back and refuse a file changed since it was read or reached through a symbolic link below the scanned path. |
| `workflow` | YAML source | A `yamlpath::Document`, the semantic YAML value, and its `Kind`; `parse` validates a workflow's `on`/`jobs` structure and `parse_action` an action's `runs` mapping and string `runs.using`. A parse error is reported for that file, the other files are still checked, and the run exits with status 2 without writing. |
| `config` | Scanned path, a remote tree, or `--config` | Per-rule `disable`, `ignore`, severity remaps, and `config` options, using zizmor's schema and discovery order; a remote repository is searched at its root only, as zizmor does. |
| `tree` | Local repository root, or a GitHub repository and commit | The `Tree` read interface (entry kind, text, file names, identity) over `/`-separated repository paths, with `LocalTree` and `RemoteTree`. `RemoteTree` answers existence from one recursive Git tree request; when GitHub truncates it, from one non-recursive request per directory a lookup needs, starting at the root. Symbolic links and submodules of a remote tree are neither files nor directories. Also the action discovery walk shared by local and remote inputs. |
| `rules` | Parsed workflow and settings | Findings with rule ID, curated tag, severity, location, route, and an optional fix proposal. Rules do not write files. |
| `rules::common` | Parsed workflow or action | Shared traversal: jobs (a composite action's `runs` is one job, with locations `action` and `step N`), steps (including `parallel` groups), triggers, `uses` refs, local actions, action versions, and boolean inputs. |
| `rules::expression` | Text with `${{ }}` | Expression spans and bodies, context paths, which of them an outsider controls, where a named secret is read, and how a condition compares or negates a path. |
| `rules::shell` | `run` script | Simple commands (quoted text and heredoc bodies stay data), the program each runs past keywords, assignments, and wrappers such as `sudo`/`env`, getopt-style options, git subcommands, and the shell a step runs in. |
| `github` | API path | Cached read-only GitHub REST responses for online rules, remote inputs, and followed actions, failures included: `commit` resolves a ref to a SHA and `contents` reads a file's raw text at a commit. Each path has its own lock, so parallel workflows request it once; after a rate-limit refusal or a 401 no further request is sent. The tag walk fails when a page repeats the previous one or after 100 pages. |
| `engine` | Source, its kind, and scan/fix mode | Findings, the local actions the file calls (`uses: ./path`), the actions of other repositories its steps call (`rules::Call`, with the span and ignore comments of each line), and a prepared fix result. This is the main in-memory analysis interface, for a file from any source, a followed action included. Files and `.github` directories are inspected on every core; results keep path order, and fixes are written only after all inspections finish. |
| `follow` | The scanned files of one config, a GitHub client, and settings | `--follow-actions`: resolves each call's ref, reads `action.yml` or `action.yaml` at the commit, inspects it as an action with the offline rules and without its own ignore comments, and follows composite calls level by level (`$/path` in the action's repository; not `./`, `docker://`, `${{ }}`, or a job's reusable workflow). Each `(owner, repo, path, sha)` is read once; the runner's `CompositeActionsMaxDepth` caps the depth. Returns one report per action with the shortest chain from each scanned `uses:` line that reaches it, the findings each chain reports after that line's inline and config ignores, and the errors; a rate limit or 401 stops the walk. |
| `fix` | Parsed workflow and proposals | Reparsed source after each patch, proof that the target disappeared, no new finding appeared, and only intended YAML nodes changed. A rejected proposal keeps its finding, records the reason, and the next proposal still runs. |
| `report` | Per-file and repository findings | Text, JSON, SARIF, annotation, and agent renderings of the same findings, plus the rule catalog as text or JSON. A followed action's finding is shown in its own file in text and JSON, with its chains (`Reached`); SARIF, annotations, and agent lines place it on the scanned `uses:` line its first chain starts at. |
| `policy` | Persona, severity, and `fail-on` | Which rules run, and whether reported findings make the command fail. |
| `migrate` | Workflows and the tool to replace | Rewrites zizmor, actionlint, poutine, or ghalint Action steps and install/run shell steps to the crosgha Action, and writes `crosgha.yml` from the tool's config. Every file is rewritten in memory and re-parsed before any is written. |
| `hook` | Current git repository | Writes or removes the `pre-push` hook that runs the CLI. Refuses to install into a hooks directory outside the repository, and never touches a hook that differs from every script a release has written (`TEMPLATES`). |
| `cli` | Command-line options | Preflight, local and remote scans, per-input configs, output, exit policy, and the decision to persist verified fixes. |

`src/main.rs` only enters the CLI. The library modules hold the behavior so
additional entry points can reuse the same analysis path.

## Rule contract

Each rule lives in `src/rules/` and declares:
- `id`: stable, and the name used in ignores and the config.
- `tag`: one from the curated smell taxonomy (`TAXONOMY` in the workspace's `build_matrix.py`).
- `summary`: one sentence, used as SARIF's descriptions and help.
- `severity`: a SARIF level.
- `persona`: the lowest persona that runs it.
- `equivalents`: `(tool, id)` pairs for the same concept in zizmor, actionlint, or ghlint.
- `configurable`: whether it reads `rules.<id>.config`.
- `relational`: whether its findings compare steps with each other, so fixing one step may clear
  or restore them; fix validation tolerates that change.

Equivalent IDs keep existing ignore comments and `zizmor.yml` entries working after a switch, and
the report names them. The concept and primary source are cited above each rule. Rules come in
four shapes, dispatched from explicit tables in `rules/mod.rs`. Each row of a file table also says what the
check reads (`Reads`), which decides the files it runs on:

| `Reads` | Runs on | For checks of |
|---|---|---|
| `Workflow` | workflows | triggers, jobs, `permissions`, `defaults`, runners, and timeouts, which an action does not have |
| `Steps` | workflows and actions | steps, `uses` lines, and text anywhere in the file; a composite action's `runs` is one job |
| `Metadata` | actions | `runs.using`, `runs.image`, the required keys, and the keys of composite steps |

A rule may have rows of several kinds (`schema-validity` has a workflow and a metadata row), and the catalog's
`actions` field lists the kinds other than `Workflow`. A `Steps` check meets syntax that differs in actions only where
a composite action has no equivalent: it skips job names and IDs, `defaults`-based shell checks, and the
`persist-credentials` fix, whose later steps are in the caller's workflow. `script-injection` also treats a
composite action's `inputs` as untrusted, as zizmor does.

| Shape | Signature | Registered in |
|---|---|---|
| Workflow | `fn(&Workflow) -> Result<Vec<Finding>, String>` | `REGISTRY` |
| Configurable | `fn(&Workflow, Options) -> Result<Vec<Finding>, String>` with its `rules.<id>.config` | `CONFIGURABLE` |
| Online | `fn(&Workflow, &GitHub, Options) -> Result<Vec<Finding>, String>` | `ONLINE`; runs only with a token |
| Repository | `fn(&dyn Tree, bool, Options) -> Result<Vec<Located>, String>` for a repository with a `.github` directory, local or remote; a `LocalTree` serves the sources the scan already read, and a `RemoteTree` the cached API answers, so no file is read twice | `REPOSITORY`; `repo-org-settings` also needs `--repository` for local inputs, and checks a remote input's own repository |

Checks that state github.com facts (retired hosted-runner images and service shutdowns in
`removed-feature`, and `retired-runtime`) sit in the `HOSTED` table and are
skipped when `--gh-hostname` names another host. The `readability` check of the workflow's file name
sits in the `NAMED` table, whose functions also take the file's base name.

A finding can have no fix when the change is not safe to choose automatically. A fix proposal
supplies a `yamlpatch` operation, whole-line rewrites of a scalar, or a scalar's new source, the
route it may change, and whether it adds, replaces, or removes that node. Line rewrites and new scalar source
keep a block scalar's indentation and header comment, which yamlpatch's `Replace` does not. The shared fix module applies and validates proposals, one at a
time, each taken from the findings of the source it patches: an earlier fix may have rewritten the lines a
later proposal names, as when one step gets a finding per ghlint concept. Detection-only rules leave source
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
   A removed key must be the only deletion: the patched source equals the original minus one
   range that holds the key, and the parsed values differ only by that key and the block
   mappings above it that it leaves empty. A flow mapping left as `{}` fails the check.
   Files with CRLF or mixed line breaks, or without a final newline, are patched as LF with
   one; then each original line gets its own break back and an added line takes the break of
   the line before it.
4. Prepare all files before writing any file. Write each changed file through
   a same-directory temporary file, preserving permissions and renaming it
   over the original. A symbolic link is never replaced or written through (any
   directory between the scanned path and the file), nor a file whose content changed
   since it was read (an edit saved during the scan). A failed write is
   reported, exits with status 2, and leaves the other files fixed; there is no
   cross-file transaction.

This validation applies to every new rule's fix. A rule that cannot describe
and verify its intended node changes remains detection-only.

## Reports and exit policy

Text (`plain` in zizmor's terms), JSON, SARIF, GitHub annotations, and agent lines render the same findings; no format runs its
own detection. JSON is versioned (`version: 1`) with a `summary` and one object per finding,
including its span and equivalents. Columns count characters in text, JSON, annotations, and
agent lines; annotations omit them on a multi-line span, as actions/toolkit requires. Under
`--fix`, `fixes` lists every proposed fix with its status: `applied`, `rejected` with the validation reason, or `not-written` with the reason the
file kept its original content. Every format reports what is on disk: the findings that remain
in a written file, and the original findings of a file whose fixes were not written. SARIF follows GitHub's
[SARIF support](https://docs.github.com/en/code-security/reference/code-scanning/sarif-files/sarif-support):
- every rule has descriptions, help, a default level, and a precision derived from its persona;
- security rules carry `security-severity`, and maintainability and correctness rules carry
  `problem.severity`;
- every result in a file has a percent-encoded relative URI (a `file:` URI outside the working
  directory) and a full region whose columns
  count UTF-16 code units (`columnKind`); a repository-wide finding names its directory in the
  message and points at line 1 of the first scanned workflow of that directory, because code
  scanning shows no result without a location;
- `partialFingerprints` is left to `upload-sarif`;
- a finding in another repository's action (`--follow-actions`) points at the scanned `uses:` line that reaches it,
  because code scanning matches locations "against a file committed to the repository" (same page), and its message
  names the action's file, line, and chain. `upload-sarif` fingerprints a result by its line, so two findings of one
  rule behind the same `uses:` line may show as one alert; text and JSON list each.

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
5. In requested PR mode, after the report, run verified fixes, commit only the
   files they wrote through a separate index (the caller's HEAD and index stay),
   push `crosgha/fix-<run_id>`, and open a PR with the supplied `fix-token`
   (GitHub App token or fine-grained PAT) that lists each rule and fix. Auto-merge is a separate explicit opt-in that fails fast when the
   base branch has no required status checks. The Action never merges directly.

Steps 1-5 are implemented in `action/entrypoint.sh` and `action.yml`. The release version comes
from the Action's own `Cargo.toml`, which release-please bumps in the tagged commit, and tags are
`vX.Y.Z` (`include-component-in-tag: false`). The entry point never fails on findings: it records
`exit-code`, the upload step runs, and a last step applies it. Step 5 runs in
`action/entrypoint.sh` too, so it precedes the upload; `tests/action.rs` runs it against a local
bare repository and a stand-in for the GitHub API (pull requests, branch rules and protection,
and the GraphQL auto-merge mutation).

## Verification surface

| Test file | What it runs |
|---|---|
| `tests/rules.rs` | Scans every `tests/fixtures/rules/<rule>[.<variant>].before.yml` at the `auditor` persona. Findings of the rule must match the `# expect: <rule>` markers exactly, `--fix` must remove all or none of them and write exactly `.after.yml` (or nothing when that file is absent; `CROSGHA_BLESS=1` regenerates it), and the hand-written `.clean.yml` must not be reported. |
| `tests/rules.rs` (online) | Runs the online fixtures against a local server (`tests/common::github_api`) that replays `tests/fixtures/online/api.json`, serves each `tests/fixtures/repos/<name>` as `fixture/<name>`, and rejects wrong tokens. |
| `tests/rules.rs` (actions) | The same checks on `tests/fixtures/actions/`, each fixture scanned as an `action.yml`. |
| `tests/rules.rs` (coverage) | Fails if any registered rule lacks a fixture, or a rule with a `metadata` check lacks an action fixture, and scans each `tests/fixtures/repos/<rule>/` repository, offline and against the fake API, which must report its rule. |
| `tests/cli.rs` | The binary end to end: inputs, discovery (a repository's local actions in `repos/composite`), exit codes, and flags. |
| `tests/fix.rs` | `--fix`: verified writes, rejected fixes, line breaks, BOMs, symlinks, and a file edited during the scan. |
| `tests/config.rs` | Config discovery (local and remote, in the same order), per-input configs, ignores, and schema errors. |
| `tests/report.rs` | Every report format and the rule catalog. |
| `tests/repository.rs` | Repository rules: Dependabot cooldown, security tooling, local actions, and github.com-only rules. Every `tests/fixtures/repos/` repository, served by the fake GitHub API (`tests/common`) at one commit, must report exactly what its local copy reports, also from a truncated tree. |
| `tests/migrate.rs`, `tests/hook.rs` | `migrate` on workflows and repositories; `hook install` and `uninstall` in scratch git repositories. |
| `tests/follow.rs` | `--follow-actions` against the fake GitHub API serving `tests/fixtures/follow/`: a chain through two composite actions, a cycle, an `action.yaml`-only action, JavaScript and Docker metadata checks, ignores on the scanned line and in the config, every format, a remote input, the depth limit, failed reads and a rate limit, the exact requests, and the flag's preconditions. `CROSGHA_TEST_HOSTED`, which only debug builds read, runs the github.com-only checks against the fake host. |
| `tests/action.rs` | `action/entrypoint.sh` with a runner environment against a local release server: verified install, SARIF and outputs, a tampered checksum, `fail-on-no-inputs`, and `follow-actions` against the fake GitHub API. |
| `tests/install.rs` | `install.sh` piped into `sh` against the same local release server: the latest release on the user's `PATH` and in-place upgrades, a tampered or missing release, a new binary that cannot run, plain HTTP only for a loopback origin, and platforms without a release binary. |

The pre-push hook and CI run formatting, Clippy, the tests, and crosgha itself on
`.github/workflows` at the `pedantic` persona in offline mode.
