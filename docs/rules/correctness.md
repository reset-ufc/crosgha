# Correctness

These rules check that a file is what GitHub will accept and run: valid keys and references, shell scripts that
write to the environment files as the runner expects, URLs with an encrypted scheme, and `gh` commands with the
environment they need. None of them looks for an attacker; they catch files that fail or misbehave on their own.

The output below shows only the findings of the rule being explained. The command ran with `--persona auditor`, so
the same run also reports naming, runtime-monitoring, trigger, and permission findings, which the summary line counts.

## `schema-validity`

Workflows and actions must use only the keys GitHub's syntax defines.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| error | regular | workflow | none | actionlint: `syntax-check`, `expression`; ghlint: `JsonSchemaValidation` |

**What it reports.** In a workflow: an unknown key on the workflow, a job, or a step; a job that calls a reusable
workflow and sets a key only a job with steps may set; an `if` on a job or a step that reads `secrets`; a `matrix.`
or `steps.` reference that names no key of the job's matrix and no step `id` in the job, skipping a matrix built from
an expression; a job with neither `runs-on` nor `uses`; a `uses` value, for a job or a step, that holds `${{ }}`; a
step `id` used twice in the same list of steps, where a `parallel` block is its own list and ids compare exactly; a
step that does not have exactly one of `uses`, `run`, `parallel`, `wait`, `wait-all`, or `cancel`; a `needs` entry
naming a job that does not exist, or one that never starts because of a dependency cycle; more than 25
`workflow_dispatch` inputs; `queue: max` combined with `cancel-in-progress: true`; and a `permissions` block with a
shorthand other than `read-all` or `write-all`, an unknown scope, or a level the scope does not accept (`id-token`
and `copilot-requests` take `write` or `none`; `models` and `vulnerability-alerts` take `read` or `none`; the others
take `read`, `write`, or `none`). In an `action.yml`: a missing `name` or `description`; a `runs.using` that is
missing or outside composite, docker, node12, node16, node20, and node24, or the key that runtime requires
(`runs.steps` as a list for composite, `runs.image` for docker, `runs.main` for the node runtimes); a composite step
with neither or both of `uses` and `run`; a key that a composite `run` or `uses` step cannot set; and a composite
`run` step without `shell`.

**Why it matters.** GitHub rejects a workflow or action file that the syntax does not define, so the run never starts.
The key sets come from the
[workflow syntax reference](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax) and,
for actions, the [metadata syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/metadata-syntax);
the runner refuses an unknown `using`, a JavaScript action without `main`, a Docker action without `image`, and a
composite action without steps.

**Example.** A workflow with a shorthand permission, an unknown job key, a step that is both a `uses` and a `run`
step, an unknown step key, a job without `runs-on`, and a `needs` on a job that does not exist:

```yaml
name: Schema
on: [push]
permissions: read
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    timeout: 10
    steps:
      - uses: example/action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        run: echo both
      - run: echo ok
        working-dir: src
  orphan:
    needs: ghost
    steps:
      - run: echo orphan
```

```text
error[schema-validity]: `permissions: read` must be `read-all`, `write-all`, or a map of scopes
 --> .github/workflows/example.yml:3:1
  |
3 | permissions: read
  | ^^^^^^^^^^^ workflow
  |
  = help: Workflows and actions must use only the keys GitHub's syntax defines.
  = note: taxonomy tag `syntax/schema-validity`; same as actionlint: syntax-check, expression; ghlint: JsonSchemaValidation

error[schema-validity]: unknown job key `timeout`
 --> .github/workflows/example.yml:8:5
  |
8 |     timeout: 10
  |     ^^^^^^^ job `build`
  |
  = help: Workflows and actions must use only the keys GitHub's syntax defines.
  = note: taxonomy tag `syntax/schema-validity`; same as actionlint: syntax-check, expression; ghlint: JsonSchemaValidation

error[schema-validity]: step needs exactly one of `uses`, `run`, `parallel`, `wait`, `wait-all`, or `cancel`
  --> .github/workflows/example.yml:10:9
   |
10 |       - uses: example/action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ job `build`, step 1
   |
   = help: Workflows and actions must use only the keys GitHub's syntax defines.
   = note: taxonomy tag `syntax/schema-validity`; same as actionlint: syntax-check, expression; ghlint: JsonSchemaValidation

error[schema-validity]: unknown step key `working-dir`
  --> .github/workflows/example.yml:13:9
   |
13 |         working-dir: src
   |         ^^^^^^^^^^^ job `build`, step 2
   |
   = help: Workflows and actions must use only the keys GitHub's syntax defines.
   = note: taxonomy tag `syntax/schema-validity`; same as actionlint: syntax-check, expression; ghlint: JsonSchemaValidation

error[schema-validity]: job needs `runs-on` or `uses`
  --> .github/workflows/example.yml:14:3
   |
14 |   orphan:
   |   ^^^^^^ job `orphan`
   |
   = help: Workflows and actions must use only the keys GitHub's syntax defines.
   = note: taxonomy tag `syntax/schema-validity`; same as actionlint: syntax-check, expression; ghlint: JsonSchemaValidation

error[schema-validity]: `needs` names `ghost`, which is not a job in this workflow
  --> .github/workflows/example.yml:15:5
   |
15 |     needs: ghost
   |     ^^^^^ job `orphan`
   |
   = help: Workflows and actions must use only the keys GitHub's syntax defines.
   = note: taxonomy tag `syntax/schema-validity`; same as actionlint: syntax-check, expression; ghlint: JsonSchemaValidation

Checked 1 workflow file: 20 findings (6 errors, 14 notes).
```

An `action.yml` read as a composite action, with no `description`, a `run` step without `shell`, and a `with` on a
`run` step:

```yaml
name: Build and test
runs:
  using: composite
  steps:
    - name: Build
      run: make build
    - name: Package
      shell: bash
      run: make package
      with:
        target: all
```

```text
error[schema-validity]: action has no `description`
 --> action.yml:2:1
  |
2 | runs:
  | ^^^^ action
  |
  = help: Workflows and actions must use only the keys GitHub's syntax defines.
  = note: taxonomy tag `syntax/schema-validity`; same as actionlint: syntax-check, expression; ghlint: JsonSchemaValidation

error[schema-validity]: composite `run` steps need `shell`
 --> action.yml:6:7
  |
6 |       run: make build
  |       ^^^ step 1
  |
  = help: Workflows and actions must use only the keys GitHub's syntax defines.
  = note: taxonomy tag `syntax/schema-validity`; same as actionlint: syntax-check, expression; ghlint: JsonSchemaValidation

error[schema-validity]: a composite `run` step cannot set `with`
  --> action.yml:10:7
   |
10 |       with:
   |       ^^^^ step 2
   |
   = help: Workflows and actions must use only the keys GitHub's syntax defines.
   = note: taxonomy tag `syntax/schema-validity`; same as actionlint: syntax-check, expression; ghlint: JsonSchemaValidation

Checked 1 action file: 3 findings (3 errors).
```

Compliant, the same workflow:

```yaml
name: Schema
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: example/action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - run: echo ok
        working-directory: src
  orphan:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    needs: build
    steps:
      - run: echo orphan
```

Compliant, the same action:

```yaml
name: Build and test
description: Builds and packages the project.
runs:
  using: composite
  steps:
    - name: Build
      shell: bash
      run: make build
    - name: Package
      shell: bash
      working-directory: build
      run: make package
```

**How to fix.** Correct the key, the reference, the scope name, or the level to the one the syntax defines: most
findings are one spelling away from a valid file (`working-dir` for `working-directory`, `timeout` for
`timeout-minutes`, `pull-request` for `pull-requests`). A duplicate step `id`, a circular `needs`, or a `secrets`
read in `if` needs another shape: rename one of the steps, break the cycle, and pass the secret through `env` and
test that instead. `--fix` does not rewrite a file it cannot prove a valid replacement for. To ignore a finding,
see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#schema-validity).

## `embedded-script-lint`

Shell scripts must quote, brace, and append to environment file paths.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | pedantic | workflow | [`--fix`](../fixes.md) | actionlint: `shellcheck`; ghlint: `SafeEnvironmentFileRedirect`, `EnvironmentFileOverwritten` |

**What it reports.** In a step whose shell resolves to bash or sh, two findings. First, an expansion of
`GITHUB_ENV`, `GITHUB_OUTPUT`, `GITHUB_PATH`, `GITHUB_STATE`, or `GITHUB_STEP_SUMMARY` that sits outside double
quotes, or a redirect target that is quoted but not braced: ShellCheck's SC2086 and SC2250. That form is reported
wherever the name appears in the script, a redirect target or not. Second, a `>` that truncates one of those files
instead of appending to it, `GITHUB_STEP_SUMMARY` included. The shell comes from the step's `shell`, the job's or
workflow's `defaults.run.shell`, or the runner default, so pwsh, cmd, and Python steps are never reported.

**Why it matters.** An unquoted path splits into several words when it holds a space or a glob character, and the
write goes to the wrong file or fails; `>` discards everything the step wrote earlier, so outputs, variables, and
`PATH` entries are lost before the runner reads the file. The sources are ShellCheck's
[SC2086](https://www.shellcheck.net/wiki/SC2086) and
[SC2250](https://www.shellcheck.net/wiki/SC2250), and ghlint's `SafeEnvironmentFileRedirect` and
`EnvironmentFileOverwritten` rules.

**Example.**

```yaml
name: Output
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo "x=1" >> $GITHUB_OUTPUT
```

```text
note[embedded-script-lint]: quote environment file paths to prevent word splitting (SC2086)
 --> .github/workflows/example.yml:9:9
  |
9 |       - run: echo "x=1" >> $GITHUB_OUTPUT
  |         ^^^ job `build`, step 1
  |
  = help: Shell scripts must quote, brace, and append to environment file paths.
  = note: same as actionlint: shellcheck; ghlint: SafeEnvironmentFileRedirect, EnvironmentFileOverwritten

Checked 1 workflow file: 7 findings (7 notes).
1 finding has a fix: rerun with --fix.
```

Compliant:

```yaml
name: Output
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo "x=1" >> "${GITHUB_OUTPUT}"
```

**How to fix.** Write every environment file path as `"${GITHUB_OUTPUT}"`: quoted, so the shell keeps it one word,
and braced, so the name is unambiguous. Append with `>>` where an earlier write must survive; the runner gives each
step new, empty files, so `>` and `>>` differ only after the step has written. `--fix` applies both rewrites (changing
only the path or only the operator) and leaves the finding when the script holds a path it cannot rewrite, such as a
command argument, a `for` list, a pattern, text in quotes, a heredoc body, or a folded YAML scalar; it also leaves
`GITHUB_STEP_SUMMARY`, `&>`, `<>`, and a script with `[[` or `$[` unfixed. The full list is in
[Fixes](../fixes.md). To ignore a finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#embedded-script-lint).

## `insecure-protocol`

URLs must use an encrypted scheme.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | none | zizmor: `insecure-url-scheme` |

**What it reports.** Any string in the file that holds `http://`, `git://`, or `ftp://`, one finding per YAML value:
a `run` script, an `env` value, an input, a description. Plain `http://` to a host that stays on the runner is not
reported: `localhost`, `127.0.0.1`, `0.0.0.0`, `::1`, `+`, `*`, and a single-label name such as a service container,
which public DNS never resolves. `https://` is never reported.

**Why it matters.** zizmor's `insecure-url-scheme` audit and GASH's "Unsecured Protocol" smell both record the risk:
a plain HTTP or Git URL can be read and changed in transit, so the script or archive that arrives is not the one
that was published.

**Example.**

```yaml
name: Fetch data
on: [push]
permissions: {}
jobs:
  fetch:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: curl -fsS -o data.json http://example.com/data.json
```

```text
warning[insecure-protocol]: URL uses an unencrypted scheme
 --> .github/workflows/example.yml:9:9
  |
9 |       - run: curl -fsS -o data.json http://example.com/data.json
  |         ^^^ workflow
  |
  = help: URLs must use an encrypted scheme.
  = note: same as zizmor: insecure-url-scheme

Checked 1 workflow file: 7 findings (1 warning, 6 notes).
```

Compliant:

```yaml
name: Fetch data
on: [push]
permissions: {}
jobs:
  fetch:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: curl -fsS -o data.json https://example.com/data.json
```

**How to fix.** Change the scheme to `https://`, `ssh://`, or `git+ssh://`, or use the `git@host:owner/repo` form,
and confirm the server offers the encrypted scheme before switching. For a downloaded script, also check its digest
before running it. `--fix` does not rewrite it, because the host must already serve the new scheme. To ignore a
finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#insecure-protocol).

## `gh-cli-environment`

Steps that run `gh` must set its token, and a repository for commands that read one.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | none | ghlint: `MissingGhToken`, `MissingGhHost`, `MissingGhRepo` |

**What it reports.** In a step whose shell resolves to bash, sh, pwsh, powershell, or cmd, a `gh` command that lacks
part of its environment, one finding per missing part at the first command that lacks it. The token check covers the
commands that run only with one (`api`, `issue`, `pr`, `release`, `repo`, `run`, `search`, `workflow`, and the other
authenticated groups) and skips `auth`, `config`, `version`, `alias`, the extension commands, `release download`,
`attestation trusted-root`, and `attestation verify` with `--bundle`; a `gh auth login` earlier in the job satisfies
it. A step that sets `GH_ENTERPRISE_TOKEN` without `GH_HOST` is reported instead as missing the host. The repository
check covers the commands that read one and is satisfied by an earlier checkout, `git clone`, `gh repo clone`,
`GH_REPO`, or `--repo`; it is skipped for `gh api` without a `{owner}`, `{repo}`, or `{branch}` placeholder, for
`gh repo` subcommands that act on the current repository, for organization or user secrets, variables, and rulesets,
and for an operand that may be a URL or an expression. The environment it reads is the workflow's, the job's, the
container's, and the step's `env`, plus variables the script exports and `GITHUB_ENV` writes of earlier steps; a step
`env` given an expression sets an unknown set of variables, which satisfies both checks.

**Why it matters.** `gh` exits with an authentication error or falls back to a repository it infers from the git
remote, so a step that looks correct fails at run time or acts on the wrong repository. ghlint splits the same
checks into `MissingGhToken`, `MissingGhHost`, and `MissingGhRepo`; the variables and their precedence are in
`gh help environment`.

**Example.**

```yaml
name: Triage
on: [issues]
permissions: {}
jobs:
  triage:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: gh pr list --state open
```

```text
warning[gh-cli-environment]: `gh pr list` runs without a token: set `GH_TOKEN: ${{ github.token }}` in `env`
 --> .github/workflows/example.yml:9:9
  |
9 |       - run: gh pr list --state open
  |         ^^^ job `triage`, step 1
  |
  = help: Steps that run `gh` must set its token, and a repository for commands that read one.
  = note: taxonomy tag `embedded-script-lint`; same as ghlint: MissingGhToken, MissingGhHost, MissingGhRepo

warning[gh-cli-environment]: `gh pr list` runs without a repository: check out the code first, set `GH_REPO: ${{ github.repository }}` in `env`, or pass `--repo`
 --> .github/workflows/example.yml:9:9
  |
9 |       - run: gh pr list --state open
  |         ^^^ job `triage`, step 1
  |
  = help: Steps that run `gh` must set its token, and a repository for commands that read one.
  = note: taxonomy tag `embedded-script-lint`; same as ghlint: MissingGhToken, MissingGhHost, MissingGhRepo

Checked 1 workflow file: 6 findings (2 warnings, 4 notes).
```

Compliant:

```yaml
name: Triage
on: [issues]
permissions: {}
jobs:
  triage:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    env:
      GH_TOKEN: ${{ github.token }}
      GH_REPO: ${{ github.repository }}
    steps:
      - run: gh pr list --state open
```

**How to fix.** Set `GH_TOKEN: ${{ github.token }}` in the step or job `env`, and give the token the permission the
command needs in `permissions`. For commands that read a repository, either check out the code first, set
`GH_REPO: ${{ github.repository }}`, or pass `--repo`. On GitHub Enterprise Server, set `GH_HOST` next to the
Enterprise token. `--fix` does not rewrite it, because the token scope belongs to the job. To ignore a finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#gh-cli-environment).
