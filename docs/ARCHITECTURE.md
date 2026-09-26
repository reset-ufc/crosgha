# Architecture

`crosgha` has one analysis path for local CLI use and the planned GitHub Action.
The Action will install a verified release binary and invoke the CLI; it will not
contain a second detector or fixer.

| Area | Current state |
|---|---|
| CLI pipeline | Structured and runnable, including `fail-on` exit policy. |
| Rule modules | Five implemented rules; shared finding and fix proposal types. |
| Fix validation | Runnable for add and replace proposals. |
| Reports | Text works; JSON and SARIF fail explicitly. |
| Composite Action | Inputs, outputs, and input guard exist; execution stops before installation. |
| PR and auto-merge | Mode contract and guards exist; repository operations are pending. |

## Execution path

```text
CLI / future Action
    -> discover workflow files
    -> parse and validate each workflow
    -> run registered rules
    -> collect findings and optional fix proposals
    -> validate every proposed patch
    -> write verified files when --fix is requested
    -> render the report and choose an exit status
```

The stages have these interfaces:

| Module | Input | Output and responsibility |
|---|---|---|
| `filesystem` | File or directory path | Sorted `.yml`/`.yaml` paths, file contents, and per-file verified writes. |
| `workflow` | YAML source | A `yamlpath::Document`, semantic YAML value, and the validated `on`/`jobs` structure. Parsing errors stop that file. |
| `rules` | Parsed workflow | Findings with rule ID, curated tag, location, message, and an optional fix proposal. Rules do not write files. |
| `engine` | Source and scan/fix mode | Findings and a prepared fix result. This is the main in-memory analysis interface. |
| `fix` | Parsed workflow and proposals | Reparsed source after each patch, proof that the target disappeared, no new finding appeared, and only intended YAML nodes changed. |
| `report` | Per-file inspection results | Text now; JSON and SARIF share this result model when implemented. |
| `policy` | Finding severity and `fail-on` setting | Whether reported findings make the command fail. |
| `cli` | Command-line options | Preflight, output, exit policy, and the decision to persist verified fixes. |

`src/main.rs` only enters the CLI. The library modules hold the behavior so
additional entry points can reuse the same analysis path.

## Rule contract

Each implemented rule belongs in `src/rules/` and uses a stable rule ID and a
tag from the curated smell taxonomy. A detector reads a parsed workflow and
returns findings. A finding can have no fix when the change is not safe to
choose automatically. A fix proposal supplies a `yamlpatch` operation, the
route it may change, and whether it adds or replaces that node. The shared
fix module applies and validates proposals. Rule concepts cite primary
sources in the rule code. `persist-credentials` has a verified fix.
`mutable-action-reference`, `missing-job-timeout`, `implicit-token-permissions`,
and `inherited-workflow-secrets` are opt-in, detection-only policy checks. Other
taxonomy tags are not registered until their detection semantics are specified
and tested.

Rules are invoked from `rules::detect_all`. This explicit dispatch keeps one
analysis path without a trait or registry framework. The CLI passes the same
rule policy through initial detection, fix validation, and final detection.
Add a rule there when its before/after workflow fixtures and binary-level test
exist. Detection-only rules leave source unchanged under `--fix` and remain
visible in the report.

## Fix safety

1. Parse the patched YAML and validate its workflow structure.
2. Run every implemented rule again. The target must disappear; no new
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

Text is the current working format. `--format json` and `--format sarif` are
reserved and fail before scanning or writing. Both will render the same
per-file findings used by text; they must not run separate detectors. A future
SARIF renderer must include stable rule IDs and source locations. GitHub's
[SARIF support](https://docs.github.com/en/code-security/reference/code-scanning/sarif-files/sarif-support)
uses `note`, `warning`, and `error` rule levels. `crosgha` assigns a level per
implemented rule and uses it for `--fail-on`. `any` is the default and `none` disables
failure on findings. `--fix` evaluates any remaining findings after fixes.
Input and unsupported-format errors exit with status 2.

## GitHub Action path

The composite Action in [ROADMAP.md](../ROADMAP.md) is a distribution layer
over this CLI. Its intended sequence is:

1. Validate Linux runner, mode, token requirements, and incompatible inputs.
2. Resolve the release version associated with the action revision. Download
   the matching Linux asset and verify its release `.sha256` before execution.
3. Run the CLI once and produce findings, job summary, and SARIF output.
4. Optionally upload SARIF using a SHA-pinned upload action; the caller grants
   `security-events: write` only when upload is enabled.
5. In requested PR mode, run verified fixes, create a branch and PR with the
   supplied GitHub App token or fine-grained PAT, and list each rule/fix.
   Auto-merge is a separate explicit opt-in and requires the checks described
   in the roadmap. The Action never merges directly.

The root `action.yml` records the planned inputs and outputs. Its entry point
currently validates Linux and mode requirements, then fails explicitly before
any download, upload, or repository write. The Action is a scaffold until the
SARIF renderer and release-version resolution are implemented
and exercised end to end. Report mode is the first Action slice; PR and
auto-merge modes build on it.

## Verification surface

`tests/cli.rs` runs the built binary against reproducible workflow fixtures.
Each implemented rule needs a before/after YAML pair and an exact fixed-output
assertion. The pre-push hook runs formatting, Clippy, and the tests. Once the
rule set is stable enough, CI and the hook can scan this repository's own
workflows as described in the roadmap.
