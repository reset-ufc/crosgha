# Architecture

`crosgha` has one analysis path for local CLI use and the planned GitHub Action.
The Action will install a verified release binary and invoke the CLI; it will not
contain a second detector or fixer.

| Area | Current state |
|---|---|
| CLI pipeline | Runnable: personas, `fail-on`, zizmor-compatible flags, and `migrate`. |
| Rule modules | 46 rules covering every taxonomy tag: 38 per workflow, 3 per repository, 5 online. |
| Config and ignores | `crosgha.yml` or `zizmor.yml` (disable, ignore, remap, and per-rule options), and inline `crosgha:` or `zizmor:` ignore comments. |
| Fix validation | Runnable for add and replace proposals; only `persist-credentials` proposes a fix. |
| Reports | Text, versioned JSON, SARIF 2.1.0, GitHub annotations (default under `GITHUB_ACTIONS=true`), and brief agent lines from the same findings; `--sarif-file` adds SARIF to any format. |
| Composite Action | Report mode works: zizmor-action's inputs, a sha256-verified release binary, SARIF upload, job summary, then `fail-on`. |
| PR and auto-merge | Mode contract and guards exist; repository operations are pending. |

## Execution path

```text
CLI / future Action
    -> discover workflow files and the config
    -> parse and validate each workflow
    -> run the rules enabled by persona and config (online rules only with a token)
    -> drop ignored findings, remap severities, apply --min-severity
    -> validate every proposed patch
    -> write verified files when --fix is requested
    -> run repository rules for the .github directory next to the scanned workflows
    -> render the report and choose an exit status
```

The stages have these interfaces:

| Module | Input | Output and responsibility |
|---|---|---|
| `filesystem` | File or directory path | Sorted `.yml`/`.yaml` paths, file contents, the owning `.github` directory, and per-file verified writes. |
| `workflow` | YAML source | A `yamlpath::Document` and semantic YAML value; `parse` also validates the `on`/`jobs` structure. Any parse error stops the run with status 2 before any write. |
| `config` | Scanned path or `--config` | Per-rule `disable`, `ignore`, severity remaps, and `config` options, using zizmor's schema and discovery order. |
| `rules` | Parsed workflow and settings | Findings with rule ID, curated tag, severity, location, route, and an optional fix proposal. Rules do not write files. |
| `rules::common` | Parsed workflow | Shared traversal: jobs, steps (including `parallel` groups), triggers, expressions, untrusted contexts, and `uses` refs. |
| `github` | API path | Cached read-only GitHub REST responses for online rules. |
| `engine` | Source and scan/fix mode | Findings and a prepared fix result. This is the main in-memory analysis interface. |
| `fix` | Parsed workflow and proposals | Reparsed source after each patch, proof that the target disappeared, no new finding appeared, and only intended YAML nodes changed. |
| `report` | Per-file and repository findings | Text, JSON, SARIF, annotation, and agent renderings of the same findings, plus the rule catalog as text or JSON. |
| `policy` | Persona, severity, and `fail-on` | Which rules run, and whether reported findings make the command fail. |
| `migrate` | Workflows and the tool to replace | Rewrites zizmor or actionlint Action steps and install/run shell steps to the crosgha Action, and writes `crosgha.yml` from the tool's config. Every file is rewritten in memory and re-parsed before any is written. |
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
| Repository | `fn(&Path, bool, Options) -> Vec<Located>` for the `.github` directory | `REPOSITORY`; `repo-org-settings` also needs `--repository` |

A finding can have no fix when the change is not safe to choose automatically. A fix proposal
supplies a `yamlpatch` operation, the route it may change, and whether it adds or replaces that
node. The shared fix module applies and validates proposals. Detection-only rules leave source
unchanged under `--fix` and remain visible in the report.

## Fix safety

1. Parse the patched YAML and validate its workflow structure.
2. Run every enabled rule again. The target must disappear; no new
   finding may appear.
3. Reconstruct the original source by reversing the declared node change.
   The result must match the original bytes, including comments and spacing.
4. Prepare all files before writing any file. Write each changed file through
   a same-directory temporary file, preserving permissions and renaming it
   over the original. An I/O failure during a later rename can leave earlier
   files fixed; there is no cross-file transaction.

This validation applies to every new rule's fix. A rule that cannot describe
and verify its intended node changes remains detection-only.

## Reports and exit policy

Text (`plain` in zizmor's terms), JSON, SARIF, GitHub annotations, and agent lines render the same findings; no format runs its
own detection. JSON is versioned (`version: 1`) with a `summary` and one object per finding,
including its span and equivalents. SARIF follows GitHub's
[SARIF support](https://docs.github.com/en/code-security/reference/code-scanning/sarif-files/sarif-support):
- every rule has descriptions, help, a default level, and a precision derived from its persona;
- security rules carry `security-severity`, and maintainability and correctness rules carry
  `problem.severity`;
- every result has a relative URI and a full region;
- `partialFingerprints` is left to `upload-sarif`.

zizmor's severities map onto the SARIF levels: `informational`/`low` → `note`, `medium` →
`warning`, `high` → `error`.
`any` is the default `--fail-on`, and `none` disables failure on findings. `--fix` evaluates
the findings that remain after fixes.

Input, parse, config, API, and write errors exit with status 2. Exit codes
deliberately stay 0/1/2 instead of zizmor's 11–14. CI steps usually check only for non-zero.

## GitHub Action path

The composite Action in [ROADMAP.md](../ROADMAP.md) is a distribution layer
over this CLI. Its inputs are a superset of `zizmorcore/zizmor-action`'s, so
`crosgha migrate --from zizmor` swaps only the `uses:` line. Its sequence is:

1. Validate Linux runner, persona, severities, modes, token requirements, and
   incompatible inputs.
2. Resolve the release version associated with the action revision. Download
   the matching Linux asset and verify its release `.sha256` before execution.
3. Run the CLI once and produce findings, job summary, and SARIF output.
4. With `advanced-security: true`, upload SARIF using a SHA-pinned upload
   action; the caller grants `security-events: write`.
5. In requested PR mode, run verified fixes, then create a branch and a PR with
   the supplied `fix-token` (GitHub App token or fine-grained PAT) that lists
   each rule and fix. Auto-merge is a separate explicit opt-in and requires the
   checks described in the roadmap. The Action never merges directly.

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
| `tests/cli.rs` | The binary end to end: fixes, exit codes, symlinks, ignores, config discovery, repository rules, report formats, and `migrate`. |
| `tests/action.rs` | `action/entrypoint.sh` with a runner environment against a local release server: verified install, SARIF and outputs, a tampered checksum, and `fail-on-no-inputs`. |

The pre-push hook and CI run formatting, Clippy, the tests, and crosgha itself on
`.github/workflows` at the `pedantic` persona in offline mode.
