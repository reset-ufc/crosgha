# Maintainability and deprecations

These rules look at how a workflow ages: runs that keep going after a newer commit, triggers that fire on
everything, settings that change nothing, jobs that grew too large to review, and features GitHub or an action
retired. Most of them only make a workflow cost more or read worse; the three deprecation rules report what GitHub
no longer honors.

## `missing-timeout`

Jobs should set `timeout-minutes`.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | pedantic | workflow | none | ghlint: `MissingJobTimeout` |

**What it reports.** Every job that sets `runs-on` and has no `timeout-minutes`. A job that calls a reusable
workflow (`uses:`) has no `runs-on`, so it is never reported. The rule reads only whether the key is present, not
its value: `timeout-minutes: 1000` passes.

**Why it matters.** Without the key, a job is cancelled only at the platform ceiling, and GitHub documents that
default as 360 minutes
([workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idtimeout-minutes)).
A step that waits forever then holds a runner for hours before anyone sees the failure.

**Example.**

```yaml
name: Build
on: workflow_dispatch
permissions: {}
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    steps:
      - name: Compile
        run: cargo build --release
        shell: bash
```

```text
note[missing-timeout]: executable job has no timeout-minutes
 --> .github/workflows/example.yml:5:3
  |
5 |   build:
  |   ^^^^^ job `build`
  |
  = help: Jobs should set `timeout-minutes`.
  = note: same as ghlint: MissingJobTimeout

Checked 1 workflow file: 2 findings (2 notes).
```

Compliant:

```yaml
name: Build
on: workflow_dispatch
permissions: {}
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 15
    steps:
      - name: Compile
        run: cargo build --release
        shell: bash
```

**How to fix.** Add `timeout-minutes` to every job, with a value a little above the job's usual run time. `--fix` does not rewrite this, because the right number depends on the job. To ignore a finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#missing-timeout).

## `error-suppression`

`continue-on-error: true` hides failures, and so do actions that pass when they find nothing to upload, attach, or publish; create-pull-request and versions before the fail-fast input have no automatic fix.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | pedantic | workflow | [`--fix`](../fixes.md) | ghlint: `FailFastUploadArtifact`, `FailFastSoftpropsGhRelease`, `FailFastPublishUnitTestResults`, `FailFastPeterEvansCreatePullRequest` |

**What it reports.** A job or a step whose `continue-on-error` is `true`, as a boolean or as the string `true`; an
expression is not reported. A step that calls one of four actions without the input that makes it fail when it
finds nothing: `actions/upload-artifact` needs `if-no-files-found`, `softprops/action-gh-release` needs
`fail_on_unmatched_files` but only when `with` sets `files`, `EnricoMi/publish-unit-test-result-action` and its
`composite`, `docker`, `linux`, `macos`, `windows`, `windows/bash`, and `windows/powershell` variants need
`action_fail_on_inconclusive`, and `peter-evans/create-pull-request` has no such input. Any value counts as set,
including `false` and `warn`, and input names match ignoring case, as the runner reads them. A `with` that is not a
literal mapping is left alone, because which inputs it sets is unknown.

**Why it matters.** ghlint names these actions in
[`FailFastActionsRule.kt`](https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/FailFastActionsRule.kt):
each of them finishes successfully when it had nothing to do, so a broken build step leaves a green run with no
artifact, no release asset, or no test report.

**Example.**

```yaml
name: Release
on: workflow_dispatch
permissions: { contents: read }
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 20
    steps:
      - name: Package
        run: make dist
        shell: bash
      - name: Upload the dist
        uses: actions/upload-artifact@v6
        with:
          name: dist
          path: dist/*.tar.gz
```

```text
note[error-suppression]: upload-artifact passes when `path` matches no files; set `if-no-files-found: error`
  --> .github/workflows/example.yml:14:9
   |
14 |         uses: actions/upload-artifact@v6
   |         ^^^^ job `build`, step 2
   |
   = help: `continue-on-error: true` hides failures, and so do actions that pass when they find nothing to upload, attach, or publish; create-pull-request and versions before the fail-fast input have no automatic fix.
   = note: same as ghlint: FailFastUploadArtifact, FailFastSoftpropsGhRelease, FailFastPublishUnitTestResults, FailFastPeterEvansCreatePullRequest

Checked 1 workflow file: 3 findings (1 warning, 2 notes).
1 finding has a fix: rerun with --fix.
```

Compliant:

```yaml
name: Release
on: workflow_dispatch
permissions: { contents: read }
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 20
    steps:
      - name: Package
        run: make dist
        shell: bash
      - name: Upload the dist
        uses: actions/upload-artifact@v6
        with:
          name: dist
          path: dist/*.tar.gz
          if-no-files-found: error
```

**How to fix.** `--fix` adds the input at the end of the step's `with`, creating `with` when the step has none
([Fixes](../fixes.md)). The step then fails where it used to pass with a warning: a missing artifact,
release asset, or test report no longer goes unnoticed. Run it on the example and the step gains one line:

```diff
       - name: Upload the dist
         uses: actions/upload-artifact@v6
         with:
           name: dist
           path: dist/*.tar.gz
+          if-no-files-found: error
```

The finding stays unfixed when the ref names a release before the input (`@v2.1.1`, or a SHA whose comment says
so), when `with:` is empty, when adding the input would re-quote a flow mapping, whether the step itself (`{ uses: ... }`) or its `with`
(`{ path: coverage/ }`), and for
`peter-evans/create-pull-request`, which has no input that fails instead. `continue-on-error: true` is never fixed:
only the author knows whether the step may fail. To ignore a finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#error-suppression).

## `missing-concurrency`

CI workflows should cancel superseded runs with a concurrency group.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | pedantic | workflow | none | zizmor: `concurrency-limits` |

**What it reports.** A workflow whose triggers include `push` or `pull_request`, with no workflow-level
`concurrency`, and with at least one job that sets no `concurrency` of its own. One workflow-level group, or a group
in every job, clears it.

**Why it matters.** GitHub
[documents](https://github.com/github/docs/blob/44f009da482ef1470b37ffea456821014d892a1f/data/reusables/actions/jobs/section-using-concurrency.md#L1)
`concurrency` as the way to keep "only a single job or workflow using the same concurrency group" running at a time.
Without it, every push keeps its own run, and the superseded ones still occupy runners until they finish.

**Example.**

```yaml
name: CI
on: [push, pull_request]
permissions: {}
jobs:
  test:
    name: Test
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Build
        run: make build
        shell: bash
```

```text
note[missing-concurrency]: superseded runs keep running without a concurrency group
 --> .github/workflows/example.yml:2:1
  |
2 | on: [push, pull_request]
  | ^^ workflow
  |
  = help: CI workflows should cancel superseded runs with a concurrency group.
  = note: same as zizmor: concurrency-limits

Checked 1 workflow file: 4 findings (4 notes).
```

Compliant:

```yaml
name: CI
on: [push, pull_request]
permissions: {}
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true
jobs:
  test:
    name: Test
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Build
        run: make build
        shell: bash
```

**How to fix.** Add a workflow-level `concurrency` with a `group` and `cancel-in-progress: true`, or give each job
its own `concurrency`. The group must name what a run supersedes: `${{ github.ref }}` for one run per branch. `--fix` does not rewrite this, because the right group depends on what the workflow does. To ignore a finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#missing-concurrency).

## `broad-trigger`

push and pull_request triggers should filter branches, tags, or paths.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | pedantic | workflow | none | none |

**What it reports.** Each `push` and `pull_request` trigger that sets none of `branches`, `branches-ignore`, `tags`,
`tags-ignore`, `paths`, and `paths-ignore`. One finding per trigger. `types` alone is not a filter: it narrows the
activity, not the branches, so a `pull_request` trigger with only `types` is still reported.

**Why it matters.** Khatami et al. (SCAM 2024) list a push trigger without branches as smell KZ-08, and Semgrep's
`github-action-push-without-branches` rule checks the same shape. Every push to every branch and every tag starts a
run, so a branch pushed only for backup costs the same as the one that is reviewed.

**Example.**

```yaml
name: CI
on:
  push:
  pull_request:
    types: [opened, synchronize]
permissions: {}
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true
jobs:
  test:
    name: Test
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Build
        run: make build
        shell: bash
```

```text
note[broad-trigger]: `push` has no branch, tag, or path filter
 --> .github/workflows/example.yml:3:3
  |
3 |   push:
  |   ^^^^ workflow
  |
  = help: push and pull_request triggers should filter branches, tags, or paths.

note[broad-trigger]: `pull_request` has no branch, tag, or path filter
 --> .github/workflows/example.yml:4:3
  |
4 |   pull_request:
  |   ^^^^^^^^^^^^ workflow
  |
  = help: push and pull_request triggers should filter branches, tags, or paths.

Checked 1 workflow file: 3 findings (3 notes).
```

Compliant:

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    types: [opened, synchronize]
    paths: ['src/**']
permissions: {}
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true
jobs:
  test:
    name: Test
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Build
        run: make build
        shell: bash
```

**How to fix.** Add `branches` or `paths` to each reported trigger to restrict runs to reviewed branches and relevant
directories. `--fix` does not rewrite this, because only the author knows which branches and paths matter. To ignore a finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#broad-trigger).

## `missing-fork-guard`

Scheduled jobs should skip forks.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | pedantic | workflow | none | none |

**What it reports.** A workflow with a `schedule` trigger, and each of its jobs that no `if` guards. A job is
guarded when its condition compares a context path that starts with `github.repository` (so `github.repository_owner`
counts too), `github.event.repository.full_name`, or `github.event.repository.owner` with `==` (or with `!=` under a negation), or tests `github.event.repository.fork`
the way that means "not a fork": `!github.event.repository.fork`, `fork == false`, or `fork != true`. Context names
match ignoring case, so `GitHub.Repository` counts. A job also inherits the guard of a job it `needs`, unless its
own `if` calls `always()`, `failure()`, or `cancelled()`, or uses `success()` with `||`: those run even when the
needed job was skipped. A workflow without a `schedule` trigger is never checked.

**Why it matters.** Khatami et al. (SCAM 2024) name running scheduled workflows on forks as smell KZ-09. Scheduled
runs carry the repository object in `github.event` since
[2022-09-27](https://github.blog/changelog/2022-09-27-github-actions-additional-information-available-in-github-event-payload-for-scheduled-workflow-runs/),
which is what makes a guard possible: without one, the job runs in a fork against that fork's repository.

**Example.**

```yaml
name: Nightly
on:
  schedule:
    - cron: '0 3 * * *'
permissions: {}
jobs:
  nightly:
    name: Nightly build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Build
        run: make nightly
        shell: bash
```

```text
note[missing-fork-guard]: scheduled job also runs in every fork
 --> .github/workflows/example.yml:7:3
  |
7 |   nightly:
  |   ^^^^^^^ job `nightly`
  |
  = help: Scheduled jobs should skip forks.

Checked 1 workflow file: 2 findings (2 notes).
```

Compliant:

```yaml
name: Nightly
on:
  schedule:
    - cron: '0 3 * * *'
permissions: {}
jobs:
  nightly:
    name: Nightly build
    if: ${{ github.repository == 'example/project' }}
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Build
        run: make nightly
        shell: bash
```

**How to fix.** Add `if: ${{ github.repository == 'owner/repo' }}` to the first job, or to every job that should not
run in a fork. Jobs that `needs` it inherit the guard, as long as their own `if` does not call a status function; a
job that must run on every outcome needs an explicit repository guard next to the status function, as in
`if: ${{ always() && github.repository == 'owner/repo' }}`. `--fix` does not rewrite this, because the repository
name is not in the workflow. To ignore a finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#missing-fork-guard).

## `missing-matrix`

CI should test more than one OS or toolchain version.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | auditor | workflow | none | none |

**What it reports.** A workflow triggered by `push` or `pull_request`, where at least one job sets `runs-on` and
runs a test command, and no job in the workflow has `strategy.matrix`. A command counts as a test when any word of
the program or its arguments is `test`, `tests`, `pytest`, `ctest`, `tox`, `nox`, `unittest`, `jest`, `vitest`,
`rspec`, or `phpunit`. Package installs are ignored (`npm install`, `pip install`, `apt install`), and so are the
shell's `test` and `[` builtins. A build that runs tests implicitly, such as `mvn package`, is not recognized.

**Why it matters.** Khatami et al. (SCAM 2024) list CI on a single OS or language version as smells KZ-19 and KZ-22:
a failure that only one platform or one runtime produces stays invisible until a user reports it.

**Example.**

```yaml
name: CI
on: [push]
permissions: {}
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true
jobs:
  test:
    name: Test
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Test
        run: cargo test
        shell: bash
```

```text
note[missing-matrix]: CI runs on a single runner and toolchain configuration
 --> .github/workflows/example.yml:7:1
  |
7 | jobs:
  | ^^^^ workflow
  |
  = help: CI should test more than one OS or toolchain version.
  = note: taxonomy tag `missing-matrix/test-diversity`

Checked 1 workflow file: 3 findings (3 notes).
```

Compliant:

```yaml
name: CI
on: [push]
permissions: {}
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true
jobs:
  test:
    name: Test
    strategy:
      matrix:
        os: [ubuntu-24.04, windows-2025, macos-15]
    runs-on: ${{ matrix.os }}
    timeout-minutes: 10
    steps:
      - name: Test
        run: cargo test
        shell: bash
```

**How to fix.** Add a `strategy.matrix` to the job that runs the tests, with the platforms or versions the project
supports, and read it in `runs-on` or in the toolchain input. `--fix` does not rewrite this, because the supported platforms are a project decision. To ignore a finding,
see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#missing-matrix).

## `cache-misuse`

Use the setup action's `cache` input instead of a separate actions/cache step.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | pedantic | workflow | none | none |

**What it reports.** In a job, an `actions/cache` step whose `with.path` lines all overlap a directory that the
job's setup action caches itself. The setup actions and the first release whose `cache` input exists are
`actions/setup-node` v2.2, `actions/setup-python` v2.3, `actions/setup-java` v2.3, `actions/setup-go` v3.2, and
`actions/setup-dotnet` v3.2; a ref below that release is ignored, a major-only ref is compared by its major, and a
SHA without a version comment counts as current. A path matches when it is the cached directory, sits inside it, or
contains it, and `/home/runner` is read as `~`. Lines starting with `!` are ignored, and a step with no path is not
reported. Only `actions/cache` is read, not `actions/cache/restore` or `actions/cache/save`.

**Why it matters.** Khatami et al. (SCAM 2024) list "use the cache parameter instead of the cache option" as smell
KZ-17. The two settings can drift: a path or key changed in the separate step leaves the setup action's own cache
restoring nothing, and the job downloads the same packages twice.

**Example.**

```yaml
name: Node
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  test:
    name: Test
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Set up Node.js
        uses: actions/setup-node@v5
      - name: Cache the npm directory
        uses: actions/cache@v5
        with:
          path: ~/.npm
          key: npm-${{ hashFiles('package-lock.json') }}
      - name: Test
        run: npm test
```

```text
note[cache-misuse]: setup action has a built-in `cache` input for this
  --> .github/workflows/example.yml:16:9
   |
16 |         uses: actions/cache@v5
   |         ^^^^ job `test`, step 2
   |
   = help: Use the setup action's `cache` input instead of a separate actions/cache step.

Checked 1 workflow file: 4 findings (2 warnings, 2 notes).
```

Compliant:

```yaml
name: Node
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  test:
    name: Test
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Set up Node.js
        uses: actions/setup-node@v5
        with:
          cache: npm
      - name: Test
        run: npm test
```

**How to fix.** Set the setup action's `cache` input and delete the `actions/cache` step. The value names the
package manager the action caches, as in `cache: npm` for `actions/setup-node`, `cache: maven` for
`actions/setup-java`, and `cache: pip` for `actions/setup-python`, or `true` for `actions/setup-go` and
`actions/setup-dotnet`. `--fix` does not rewrite this, because the cache key has to be chosen. To ignore a
finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#cache-misuse).

## `duplicated-steps`

A job should not repeat a step verbatim.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | pedantic | workflow | none | none |

**What it reports.** Within one job, a step whose keys and values, leaving out `name` and `id`, equal those of an
earlier step in the same job. The first step is kept and each later repeat is reported. A step whose ID the job
reads as `steps.<id>`, in a `${{ }}` expression or in a bare `if`, is treated as a measurement and not reported. A step
that sets `parallel` is skipped.

**Why it matters.** GASH (Freitas & Rocha, VEM 2024) names a step repeated verbatim within one job "Code Replica".
The copy has to be edited twice, and a change made to one of them leaves the job running two versions of the same
command.

**Example.**

```yaml
name: Test
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  test:
    name: Test
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Test
        run: make test
      - name: Test again
        run: make test
```

```text
note[duplicated-steps]: step repeats an earlier step in the same job
  --> .github/workflows/example.yml:16:9
   |
16 |         run: make test
   |         ^^^ job `test`, step 2
   |
   = help: A job should not repeat a step verbatim.
   = note: taxonomy tag `redundant/duplicated-steps`

Checked 1 workflow file: 2 findings (2 notes).
```

Compliant:

```yaml
name: Test
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  test:
    name: Test
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Test
        run: make test
      - name: Test the integration suite
        run: make test-integration
```

**How to fix.** Delete the repeated step, or change it so it does something else: a second test run usually means
the first one missed a suite or an argument. When the repeat is intentional because the step's output changes, give
it an `id` and read it, which is what the rule treats as a measurement. `--fix` does not rewrite this, because only the author knows which of the two steps to keep. To ignore a
finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#duplicated-steps).

## `redundant-setting`

`env` should not be empty, a `shell` should not repeat the default it inherits, and steps that share a shell should set it once in `defaults.run.shell`.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | auditor | workflow | [`--fix`](../fixes.md) | ghlint: `EmptyWorkflowEnv`, `EmptyJobEnv`, `EmptyStepEnv`, `RedundantShell`, `RedundantDefaultShell`, `DuplicateShellOnSteps` |

**What it reports.** Four shapes. An `env` written as `{}` on the workflow, a job, or a step. A step's `shell`
equal to the nearest `defaults.run.shell`, the job's or else the workflow's. A job's `defaults.run.shell` equal to
the workflow's. Steps that all set the same shell, when the default should carry it instead: two or more `run` steps
with no default at all, every `run` step overriding a differing job default, or every `run` step in the workflow
overriding a differing workflow default. Only an empty mapping counts as empty: `env:` with no value, or with only
comments under it, is null and stays, and so does an expression that yields the map at run time. A step's shell is
never compared with the runner's default, since `shell: bash` adds `--noprofile --norc -o pipefail` to the
`bash -e {0}` a runner uses when no shell is set, and Windows runners default to pwsh
([supported shells](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idstepsshell)).

**Why it matters.** GitHub merges the workflow, job, and step `env` maps, so an empty one adds nothing, and a step
without `shell` runs the nearest default, so it keeps the shell it had. The repeated keys only make a reader check
whether the value is meant to differ. ghlint's
[`EmptyEnvRule.kt`](https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/EmptyEnvRule.kt)
and
[`RedundantShellRule.kt`](https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/RedundantShellRule.kt)
report the same shapes.

**Example.**

```yaml
name: Build
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    env: {}
    steps:
      - name: Compile
        run: make
        shell: bash
```

```text
note[redundant-setting]: empty `env` sets nothing
  --> .github/workflows/example.yml:12:5
   |
12 |     env: {}
   |     ^^^ job `build`
   |
   = help: `env` should not be empty, a `shell` should not repeat the default it inherits, and steps that share a shell should set it once in `defaults.run.shell`.
   = note: taxonomy tag `redundant/duplicated-steps`; same as ghlint: EmptyWorkflowEnv, EmptyJobEnv, EmptyStepEnv, RedundantShell, RedundantDefaultShell, DuplicateShellOnSteps

note[redundant-setting]: `shell` repeats the workflow's `defaults.run.shell`
  --> .github/workflows/example.yml:16:9
   |
16 |         shell: bash
   |         ^^^^^ job `build`, step 1
   |
   = help: `env` should not be empty, a `shell` should not repeat the default it inherits, and steps that share a shell should set it once in `defaults.run.shell`.
   = note: taxonomy tag `redundant/duplicated-steps`; same as ghlint: EmptyWorkflowEnv, EmptyJobEnv, EmptyStepEnv, RedundantShell, RedundantDefaultShell, DuplicateShellOnSteps

Checked 1 workflow file: 3 findings (3 notes).
2 findings have a fix: rerun with --fix.
```

Compliant:

```yaml
name: Build
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Compile
        run: make
```

**How to fix.** `--fix` removes the key: the empty `env`, the step's redundant `shell`, or the job's redundant
`defaults.run.shell`, and with it `run` and `defaults` when nothing else is left in them. A comment on the removed
key's line goes with it; comment lines around it stay. Running it on the example removes both lines:

```diff
 jobs:
   build:
     name: Build
     runs-on: ubuntu-24.04
     timeout-minutes: 10
-    env: {}
     steps:
       - name: Compile
         run: make
-        shell: bash
```

The finding stays unfixed when removing the key would leave an empty flow mapping, as
`defaults: { run: { shell: bash } }` would keep `run: {}`, and when the removal would add or remove another
finding, as when every step of the workflow sets the shell that `DuplicateShellOnSteps` asks to make the default.
That last one is never fixed: choosing which default to set is the author's call. To ignore a finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#redundant-setting).

## `complex-job`

Workflows, jobs, scripts, and conditions should stay small enough to review.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | auditor | workflow | none | ghlint: `TooManySteps`, `TooManyJobs` |

**What it reports.** More than 10 jobs in a workflow, where a reusable workflow call counts as a job too. More than
20 steps in one job. More than 20 non-empty lines in one `run` script. More than 2 `&&` or `||` operators in a job's
`if` or in a step's `if`. Only the job and step counts have a ghlint equivalent, `TooManyJobs` and `TooManySteps`;
a long script or a complex condition is reported without one.

**Why it matters.** GASH (Freitas & Rocha, VEM 2024) names long code blocks and unnecessary complexity, and the step
and job ceilings follow ghlint's
[`ComponentCountRule.kt`](https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/ComponentCountRule.kt).
Past those sizes a reviewer stops reading and starts scrolling, and a condition with three operators is where the
branch that never runs hides.

**Example.**

```yaml
name: Deploy
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  deploy:
    name: Deploy
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Deploy
        if: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' && github.actor != 'bot' || github.run_attempt > 1 }}
        run: make deploy
```

```text
note[complex-job]: condition has 3 logical operators (limit 2)
  --> .github/workflows/example.yml:14:9
   |
14 |         if: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' && github.actor != 'bot' || github.run_attempt > 1 }}
   |         ^^ job `deploy`, step 1
   |
   = help: Workflows, jobs, scripts, and conditions should stay small enough to review.
   = note: taxonomy tag `long-job/complexity`; same as ghlint: TooManySteps, TooManyJobs

Checked 1 workflow file: 2 findings (2 notes).
```

Compliant:

```yaml
name: Deploy
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  deploy:
    name: Deploy
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Deploy
        if: ${{ github.ref == 'refs/heads/main' && github.run_attempt > 1 }}
        run: make deploy
```

**How to fix.** Move a long script into a file in the repository and call it from `run`, split a job that grew past
20 steps into jobs that `needs` each other, and replace a long condition with a named step output or a variable
that holds the decision. `--fix` does not rewrite this, because the split is a design choice. To ignore a finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#complex-job).

## `readability`

Workflows, jobs, and run steps should have names, lower-case kebab IDs, and a stated shell; conditions and token references should say what they mean.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | auditor | workflow | [`--fix`](../fixes.md) | zizmor: `anonymous-definition`; ghlint: `MissingWorkflowName`, `MissingJobName`, `MissingStepName`, `WorkflowIdNaming`, `JobIdNaming`, `StepIdNaming`, `SimilarStepId`, `MissingShell`, `PreferGitHubToken`, `DoubleCurlyIf`, `NeverUseAlways`, `NegativeStatusCheck` |

**What it reports.** A workflow with no `name`; a workflow file whose name is not lower-case kebab; a job ID or a
step ID with anything but `a-z`, `0-9`, and `-`; a job with no `name` or a blank one; a `run` step with no `name`;
a `run` step with no `shell` anywhere above it; a step ID within two edits of an earlier ID in the same job, where a
swap of two adjacent characters counts as one edit; and `secrets.GITHUB_TOKEN` in a step's values, in any casing,
but not in `if`, which cannot read `secrets`. It also reports four shapes of condition: one not wrapped in `${{ }}`,
one whose leading `!` YAML reads as a tag, one that calls `always()`, and one that negates a status check, as
`!cancelled()` does. A repeated step ID is left to `schema-validity`, and a composite action's steps must set their
own shell, so their missing shell is a schema error instead.

**Why it matters.** zizmor's `anonymous-definition` and ghlint's name, ID, shell, token, and condition rules
([`MissingNameRule.kt`](https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/MissingNameRule.kt),
[`IdNamingRule.kt`](https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/IdNamingRule.kt),
[`MissingShellRule.kt`](https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/MissingShellRule.kt),
[`PreferGitHubTokenRule.kt`](https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/PreferGitHubTokenRule.kt))
cover the same ground, and GitHub accepts every shape the rule reports. `github.token` is the name GitHub
[documents](https://docs.github.com/en/actions/reference/workflows-and-actions/contexts) for the automatic token,
which is set for a job's steps, so the secret spelling reads like a repository secret the reader has to find.

**Example.**

```yaml
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Deploy
        if: github.ref == 'refs/heads/main'
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: make deploy
```

```text
note[readability]: workflow has no `name`
 --> .github/workflows/example.yml:1:1
  |
1 | on: workflow_dispatch
  | ^^ workflow
  |
  = help: Workflows, jobs, and run steps should have names, lower-case kebab IDs, and a stated shell; conditions and token references should say what they mean.
  = note: same as zizmor: anonymous-definition; ghlint: MissingWorkflowName, MissingJobName, MissingStepName, WorkflowIdNaming, JobIdNaming, StepIdNaming, SimilarStepId, MissingShell, PreferGitHubToken, DoubleCurlyIf, NeverUseAlways, NegativeStatusCheck

note[readability]: condition is not wrapped in `${{ }}`
  --> .github/workflows/example.yml:13:9
   |
13 |         if: github.ref == 'refs/heads/main'
   |         ^^ job `build`, step 1
   |
   = help: Workflows, jobs, and run steps should have names, lower-case kebab IDs, and a stated shell; conditions and token references should say what they mean.
   = note: same as zizmor: anonymous-definition; ghlint: MissingWorkflowName, MissingJobName, MissingStepName, WorkflowIdNaming, JobIdNaming, StepIdNaming, SimilarStepId, MissingShell, PreferGitHubToken, DoubleCurlyIf, NeverUseAlways, NegativeStatusCheck

note[readability]: `secrets.GITHUB_TOKEN` reads like a repository secret; the automatic token is `github.token`
  --> .github/workflows/example.yml:15:11
   |
15 |           GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
   |           ^^^^^^^^ job `build`, step 1, `env.GH_TOKEN`
   |
   = help: Workflows, jobs, and run steps should have names, lower-case kebab IDs, and a stated shell; conditions and token references should say what they mean.
   = note: same as zizmor: anonymous-definition; ghlint: MissingWorkflowName, MissingJobName, MissingStepName, WorkflowIdNaming, JobIdNaming, StepIdNaming, SimilarStepId, MissingShell, PreferGitHubToken, DoubleCurlyIf, NeverUseAlways, NegativeStatusCheck

Checked 1 workflow file: 4 findings (4 notes).
1 finding has a fix: rerun with --fix.
```

Compliant:

```yaml
name: Build
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Deploy
        if: ${{ github.ref == 'refs/heads/main' }}
        env:
          GH_TOKEN: ${{ github.token }}
        run: make deploy
```

**How to fix.** `--fix` rewrites only the token reference: every `secrets.GITHUB_TOKEN` in a step's values becomes
`github.token`, and the scalar keeps its quotes, block header, and comments. Running it on the example changes one
line and leaves the other two `readability` findings:

```diff
         env:
-          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
+          GH_TOKEN: ${{ github.token }}
```

The finding stays unfixed when the value carries a YAML anchor or tag or is an alias, whose rewrite would change
every alias of it, and when the scalar's source spells the reference differently from its value. Values outside
steps are not reported, because `github.token` is null there, so a reusable workflow call's `with` and `secrets`,
and workflow and job `env`, keep the secret. The rest is manual: add `name` to the workflow, each job, and each
`run` step, rename IDs to lower-case kebab, wrap a bare condition in `${{ }}`, and replace `always()` with
`success() || failure()` and a negated status check with the statuses that run the step. To ignore a finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#readability).

## `deprecated-command`

Deprecated workflow commands and removed opt-out variables must not be used.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | [`--fix`](../fixes.md) | zizmor: `insecure-commands`; actionlint: `deprecated-commands` |

**What it reports.** A `run` script whose words contain `::set-output`, `::save-state`, `::set-env`, or
`::add-path`, reported once per step, with a message naming one of them. It also reports an `env` entry of
`ACTIONS_ALLOW_UNSECURE_COMMANDS` or `ACTIONS_ALLOW_USE_UNSECURE_NODE_VERSION` set to true, in any casing, or to an
expression, at workflow, job, or step level. A comment that names a command runs nothing and is not reported, and
`false` on either variable is not reported.

**Why it matters.** GitHub disabled `set-env` and `add-path` and deprecated `set-output` and `save-state` in two
changelog posts
([2020-10-01](https://github.blog/changelog/2020-10-01-github-actions-deprecating-set-env-and-add-path-commands/),
[2022-10-11](https://github.blog/changelog/2022-10-11-github-actions-deprecating-save-state-and-set-output-commands/)),
and removed the Node 20 opt-out with Node 20 itself
([2026-09-23](https://github.blog/changelog/2026-09-23-node-20-is-no-longer-available-in-github-actions/)). The
replacement is an environment file, so a script that still uses a command writes its value through a channel GitHub
no longer supports, and a later step that reads it can find nothing.

**Example.**

```yaml
name: Outputs
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Version
        id: version
        run: echo "::set-output name=version::$(git describe --tags)"
```

```text
warning[deprecated-command]: `::set-output` is a deprecated workflow command
  --> .github/workflows/example.yml:15:9
   |
15 |         run: echo "::set-output name=version::$(git describe --tags)"
   |         ^^^ job `build`, step 1
   |
   = help: Deprecated workflow commands and removed opt-out variables must not be used.
   = note: taxonomy tag `deprecated-command/feature`; same as zizmor: insecure-commands; actionlint: deprecated-commands

Checked 1 workflow file: 2 findings (1 warning, 1 note).
1 finding has a fix: rerun with --fix.
```

Compliant:

```yaml
name: Outputs
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Version
        id: version
        run: echo "version=$(git describe --tags)" >> "${GITHUB_OUTPUT}"
```

**How to fix.** `--fix` rewrites each command of a bash or sh step into the environment file write that replaces it,
after [Fixes](../fixes.md). Running it on the example changes the one line:

```diff
       - name: Version
         id: version
-        run: echo "::set-output name=version::$(git describe --tags)"
+        run: echo "version=$(git describe --tags)" >> "${GITHUB_OUTPUT}"
```

| Before | After |
|---|---|
| `echo "::set-output name=x::1"` | `echo "x=1" >> "${GITHUB_OUTPUT}"` |
| `echo "::save-state name=x::1"` | `echo "x=1" >> "${GITHUB_STATE}"` |
| `echo "::set-env name=X::1"` | `echo "X=1" >> "${GITHUB_ENV}"` |
| `echo "::add-path::/opt/bin"` | `printf '%s\n' "/opt/bin" >> "${GITHUB_PATH}"` |

A step stays unfixed when it does not run bash or sh, when its script is a folded or quoted YAML scalar, when a line
is not exactly one `echo` with one argument, when the name or the value holds something the new form cannot carry
(a non-literal name, the value's own quote, a trailing `\`, an unquoted glob or substitution), when an `add-path`
path holds `\` or is unquoted with a space or `$`, when the script holds `%25`, `%0D`, or `%0A`, when the command
sits in a heredoc, a quoted string that spans lines, or a line continued with `\`, when the script uses `$'...'`
quoting, or when the rewrite would be a new finding. A value that expands to several lines at run time changes behavior, so use the
[multiline syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#multiline-strings)
for it. The opt-out variables are never fixed: delete the `env` entry, and for the Node 20 opt-out upgrade the action
to a release that runs on Node 24. To ignore a finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#deprecated-command).

## `removed-feature`

Retired runner images, shut-down action versions, and removed action inputs must not be used.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| error | regular | workflow | [`--fix`](../fixes.md) | actionlint: `runner-label`, `action`, `deprecated-inputs` |

**What it reports.** A job whose `runs-on` names a hosted runner image GitHub retired: `ubuntu-16.04`,
`windows-2016`, `macos-10.15`, `macos-11`, `macos-12`, `ubuntu-18.04`, `ubuntu-20.04`, `windows-2019`, and
`macos-13`. It reads the labels a matrix expands to, including `include` entries, and skips a job whose `runs-on`
names a runner group or contains `self-hosted`. It also reports `actions/upload-artifact` and
`actions/download-artifact` at v3 or older, `actions/cache` below v3.4.0 and v4 below v4.2.0, and
`actions/setup-java` v6 or later with `distribution: adopt`, `adopt-hotspot`, or `adopt-openj9`.

**Why it matters.** Each entry is one GitHub retired or shut down: the
[ubuntu-20.04 brownout dates](https://github.blog/changelog/2025-01-15-github-actions-ubuntu-20-runner-image-brownout-dates-and-other-breaking-changes/),
the [windows-2019 removal](https://github.blog/changelog/2025-04-15-upcoming-breaking-changes-and-releases-for-github-actions/),
the [artifact action v3 deprecation](https://github.blog/changelog/2024-04-16-deprecation-notice-v3-of-the-artifact-actions/),
and setup-java's README, which says to "use `temurin` instead of `adopt` or `adopt-hotspot`, and `semeru` instead of
`adopt-openj9`" ([README](https://github.com/actions/setup-java/blob/main/README.md)). In every case the job stops
working: the label no longer names an image, the version no longer runs, or the action no longer has that
distribution.

**Example.**

```yaml
name: Java
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Set up Java
        uses: actions/setup-java@v6
        with:
          distribution: adopt
          java-version: '17'
```

```text
error[removed-feature]: setup-java v6 removed the `adopt` distribution; use `temurin`
  --> .github/workflows/example.yml:14:9
   |
14 |         uses: actions/setup-java@v6
   |         ^^^^ job `build`, step 1
   |
   = help: Retired runner images, shut-down action versions, and removed action inputs must not be used.
   = note: taxonomy tag `deprecated-command/feature`; same as actionlint: runner-label, action, deprecated-inputs

Checked 1 workflow file: 3 findings (1 error, 1 warning, 1 note).
1 finding has a fix: rerun with --fix.
```

Compliant:

```yaml
name: Java
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Set up Java
        uses: actions/setup-java@v6
        with:
          distribution: temurin
          java-version: '17'
```

**How to fix.** `--fix` replaces an AdoptOpenJDK distribution with the successor setup-java's README names: `adopt`
and `adopt-hotspot` become `temurin`, `adopt-openj9` becomes `semeru`. Running it on the example changes one line:

```diff
       - name: Set up Java
         uses: actions/setup-java@v6
         with:
-          distribution: adopt
+          distribution: temurin
           java-version: '17'
```

The image and version findings stay unfixed: move the job to a supported image such as `ubuntu-24.04` or
`ubuntu-latest`, and upgrade the action to a release that still runs. On a GitHub Enterprise Server host (`--gh-hostname`)
the hosted parts are skipped and only the setup-java check runs ([online rules](../configuration.md#online-rules)). To
ignore a finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#removed-feature).

## `retired-runtime`

JavaScript actions must not declare a retired Node.js runtime.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | none | actionlint: `action` |

**What it reports.** A step that uses an `actions/*` action at a major whose newest release declares `node12`,
`node16`, or `node20` in `runs.using`, read from a built-in table of 20 actions. It also reports an action file
whose own `runs.using` is one of those three. The unit is the major, because `@vN` follows its newest release, so a
pin to an older minor of a current major is not reported; a ref without a version (a branch, or a SHA without a
`# vX.Y.Z` comment) and actions of other owners are not read.

**Why it matters.** GitHub retired Node 12, Node 16, and Node 20 for actions
([2023-05-04](https://github.blog/changelog/2023-05-04-github-actions-all-actions-will-run-on-node16-instead-of-node12/),
[2024-09-25](https://github.blog/changelog/2024-09-25-end-of-life-for-actions-node16/),
[2026-09-23](https://github.blog/changelog/2026-09-23-node-20-is-no-longer-available-in-github-actions/)): a runner
now runs such an action on Node 24, so it runs on a runtime its author never tested.

**Example.**

```yaml
name: Checkout
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Check out the code
        uses: actions/checkout@v4
        with:
          persist-credentials: false
      - name: Compile
        run: make
```

```text
warning[retired-runtime]: actions/checkout v4 declares `node20`, which GitHub retired; runners force it onto Node 24, so use a release built for `node24`
  --> .github/workflows/example.yml:14:9
   |
14 |         uses: actions/checkout@v4
   |         ^^^^ job `build`, step 1
   |
   = help: JavaScript actions must not declare a retired Node.js runtime.
   = note: taxonomy tag `deprecated-command/feature`; same as actionlint: action

Checked 1 workflow file: 3 findings (2 warnings, 1 note).
```

Compliant:

```yaml
name: Checkout
on: workflow_dispatch
permissions: {}
defaults:
  run:
    shell: bash
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Check out the code
        uses: actions/checkout@v5
        with:
          persist-credentials: false
      - name: Compile
        run: make
```

**How to fix.** Move the step to a major that no longer declares a retired runtime, as the example does with
`actions/checkout@v5`, and for an action of your own change `runs.using` in its `action.yml` from `node20` to
`node24` and rebuild its bundle. `--fix` does not rewrite this, because a new major can change the action's inputs.
On a GitHub Enterprise Server host (`--gh-hostname`) the check is skipped
([online rules](../configuration.md#online-rules)). To ignore a finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#retired-runtime).
