# Runners

These three rules read `runs-on` and the first steps of a job: which machine runs it, which image that machine starts
from, and whether anything watches the network while it runs.

## `runtime-monitoring`

Jobs should start with a runtime monitor such as harden-runner.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | auditor | workflow | none | none |

**What it reports.** Every job that has steps, unless its first step is `step-security/harden-runner`, matched without
case. A monitor placed anywhere but first does not count, and neither does a local action named like one. Four kinds of
job are skipped: a job whose labels include `ubuntu-slim`; a job with a `container` that is not self-hosted, because
harden-runner needs `sudo` on the VM; a job that calls a reusable workflow, which has no steps of its own; and a job
whose steps all sit inside a `parallel` block.

**Why it matters.** The rule follows StepSecurity's secure-repo check `harden-runner-integration`, cited in the rule's
[code](https://github.com/reset-ufc/crosgha/blob/master/src/rules/runner.rs), which asks for a runtime monitor as each
job's first step. The jobs it skips are the ones
[harden-runner documents](https://github.com/step-security/harden-runner/blob/main/docs/limitations.md) it cannot
monitor.

**Example.**

```yaml
name: Monitoring
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
        with: { persist-credentials: false }
```

```text
note[runtime-monitoring]: job has no runtime monitor as its first step
 --> .github/workflows/example.yml:5:3
  |
5 |   build:
  |   ^^^^^ job `build`
  |
  = help: Jobs should start with a runtime monitor such as harden-runner.

Checked 1 workflow file: 5 findings (1 warning, 4 notes).
```

Compliant:

```yaml
name: Monitoring
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: step-security/harden-runner@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          egress-policy: audit
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
        with: { persist-credentials: false }
```

**How to fix.** Make `step-security/harden-runner` the first step of every job, pinned to a full commit SHA, with the
egress policy the job needs. `--fix` does not rewrite this finding: the monitor's version and policy are a project
decision, as stated in [fixes](../fixes.md). To accept a finding, ignore it as described in
[configuration](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#runtime-monitoring).

## `runner-versioning`

`runs-on` should name a fixed image version, not `-latest`.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | pedantic | workflow | none | none |

**What it reports.** Any literal runner label that ends in `-latest`, in the string, list, and `labels` forms of
`runs-on`. A label written as `${{ matrix.<key> }}`, or as a dotted path into a matrix value such as
`${{ matrix.config.os }}`, expands to the values under `strategy.matrix` and `strategy.matrix.include[*]`, so a matrix
that holds `macos-latest` is reported too. Any other expression is left alone, since its value is unknown.

**Why it matters.** Khatami et al. (SCAM 2024), rule KZ-15 "Use fixed version for runs-on argument", cited in the
rule's [code](https://github.com/reset-ufc/crosgha/blob/master/src/rules/runner.rs), asks for a fixed runner image. A
`-latest` label follows GitHub's image rolls, so the toolchain under the job changes without a commit.

**Example.**

```yaml
name: Runner labels
on: [push]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - run: make test
```

```text
note[runner-versioning]: `-latest` runner label changes image without notice
 --> .github/workflows/example.yml:6:5
  |
6 |     runs-on: ubuntu-latest
  |     ^^^^^^^ job `test`
  |
  = help: `runs-on` should name a fixed image version, not `-latest`.

Checked 1 workflow file: 8 findings (8 notes).
```

Compliant:

```yaml
name: Runner labels
on: [push]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make test
```

**How to fix.** Name the image version the job was tested on, such as `ubuntu-24.04` or `windows-2025`, in the job and
in every matrix value it expands from. `--fix` does not rewrite this finding: the version to pin is a project decision,
as stated in [fixes](../fixes.md). To accept a finding, ignore it as described in
[configuration](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#runner-versioning).

## `self-hosted-runner`

Self-hosted runners must not run jobs from pull requests or issues.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | none | zizmor: `self-hosted-runner` |

**What it reports.** Only when the workflow runs on one of `pull_request`, `pull_request_target`,
`pull_request_review`, `pull_request_review_comment`, `issues`, `issue_comment`, `discussion`, or
`discussion_comment`. In such a workflow, a job is reported when one of its literal runner labels is one every
self-hosted runner gets by default (`self-hosted`, `linux`, `windows`, `macos`, `x64`, `arm`, `arm64`, matched without
case) or matches a configured custom label. Labels come from the string, list, and `labels` forms of `runs-on`, and a
`${{ matrix.<key> }}` label expands to its matrix values.

**Why it matters.** GitHub
[says](https://docs.github.com/en/actions/reference/security/secure-use#hardening-for-self-hosted-runners) self-hosted
runners should not serve public pull requests. A job from a pull request runs code the contributor wrote, on a machine
that keeps its credentials and state between jobs.

**Example.**

```yaml
name: Self-hosted
on: [pull_request]
permissions: {}
jobs:
  test:
    runs-on: [self-hosted, linux]
    timeout-minutes: 10
    steps:
      - run: make test
```

```text
warning[self-hosted-runner]: self-hosted runner accepts jobs from pull requests or issues
 --> .github/workflows/example.yml:6:5
  |
6 |     runs-on: [self-hosted, linux]
  |     ^^^^^^^ job `test`
  |
  = help: Self-hosted runners must not run jobs from pull requests or issues.
  = note: same as zizmor: self-hosted-runner

Checked 1 workflow file: 8 findings (1 warning, 7 notes).
```

Compliant:

```yaml
name: Self-hosted
on: [pull_request]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make test
```

**How to fix.** Run pull request and issue jobs on GitHub-hosted runners, and keep the self-hosted labels for jobs that
only trusted triggers start, such as `push` or `workflow_dispatch`. `--fix` does not rewrite this finding: it changes
where a job runs, as stated in [fixes](../fixes.md). To accept a finding, ignore it as described in
[configuration](../configuration.md#ignoring-findings).

**Options.** Custom labels of your own runners are unknown to crosgha, so list them under
`rules.self-hosted-runner.config.labels`. A `*` matches any run of characters, and a custom label matches as written,
without folding case:

```yaml
rules:
  self-hosted-runner:
    config:
      labels:
        - linux.2xlarge
        - gpu-*
```

All fixtures for this rule: [examples](../rule-examples.md#self-hosted-runner).
