# Permissions and secrets

These eight rules limit what a workflow can do with the `GITHUB_TOKEN` it is given and with the secrets it can read.
Four of them read `permissions`, one reads workflow-level `env` and the expressions that reach `secrets`, one reads
`secrets` on reusable workflow calls, and two read the steps and containers that hand credentials to something else.

## `excessive-permissions`

GITHUB_TOKEN must not get read-all, write-all, or workflow-level write scopes that several jobs inherit.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | none | zizmor: `excessive-permissions` |

**What it reports.** A workflow-level `permissions` is reported when it is the shorthand `write-all` or `read-all`, or
when it grants any `write` scope and more than one job inherits it. A job inherits the block when it declares no
`permissions` of its own; a job that calls a reusable workflow counts as two, because every job of the called workflow
gets the same scopes. A job's own block is reported when it is `write-all` or `read-all`. A workflow-level write scope
that reaches a single job is not reported: there it is as narrow as a job-level block. Read scopes are not reported.

**Why it matters.** GitHub
[recommends](https://docs.github.com/en/actions/reference/security/secure-use#using-secrets) granting `GITHUB_TOKEN`
the least access each job needs. A token that reaches every job with a write scope is a token that any compromised step
of any job can use to write to the repository.

**Example.**

```yaml
name: Permissions
on: [push]
permissions:
  contents: write
  pull-requests: read
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make build
  docs:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make docs
```

```text
warning[excessive-permissions]: write scopes at the workflow level reach every job
 --> .github/workflows/example.yml:3:1
  |
3 | permissions:
  | ^^^^^^^^^^^ workflow
  |
  = help: GITHUB_TOKEN must not get read-all, write-all, or workflow-level write scopes that several jobs inherit.
  = note: same as zizmor: excessive-permissions

Checked 1 workflow file: 13 findings (1 warning, 12 notes).
```

Compliant:

```yaml
name: Permissions
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    permissions:
      contents: read
    steps:
      - run: make build
  docs:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    permissions:
      contents: write
    steps:
      - run: make docs
```

**How to fix.** Move the scopes to the jobs that use them and grant each job only what it needs. Keep
`permissions: {}` at the workflow level so a job added later starts with nothing. `--fix` does not rewrite this finding, because choosing a job's scopes needs project context.
To ignore a finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#excessive-permissions).

## `workflow-level-permissions`

Each job should declare its own `permissions` instead of sharing a workflow-level block.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | auditor | workflow | none | ghlint: `ExplicitJobPermissions` |

**What it reports.** Only when the workflow declares `permissions` at the top, as a mapping or as a shorthand. A job
without its own block is reported when the workflow block is not narrow; narrow means empty (`{}`) or exactly
`contents: read`. A non-empty workflow block is also reported once when any job declares its own block, since
those jobs already say what they need. An empty workflow block next to job-level blocks is not reported: it grants
nothing and only guards jobs added later.

**Why it matters.** The check comes from ghlint's
[`ExplicitJobPermissions`](https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/ExplicitJobPermissionsRule.kt),
which asks for one block per job so each job gets the least access it needs. GitHub supports workflow-level
permissions, so this rule stays at the `auditor` persona as a hardening note.

**Example.**

```yaml
name: Release
on: [push]
permissions:
  contents: write
  pull-requests: read
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make
  publish:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    permissions:
      contents: write
    steps:
      - run: make publish
```

```text
note[workflow-level-permissions]: jobs declare their own `permissions`; declare them only per job
 --> .github/workflows/example.yml:3:1
  |
3 | permissions:
  | ^^^^^^^^^^^ workflow
  |
  = help: Each job should declare its own `permissions` instead of sharing a workflow-level block.
  = note: taxonomy tag `excessive-permissions`; same as ghlint: ExplicitJobPermissions

note[workflow-level-permissions]: job inherits the workflow-level `permissions`; declare its own
 --> .github/workflows/example.yml:7:3
  |
7 |   build:
  |   ^^^^^ job `build`
  |
  = help: Each job should declare its own `permissions` instead of sharing a workflow-level block.
  = note: taxonomy tag `excessive-permissions`; same as ghlint: ExplicitJobPermissions

Checked 1 workflow file: 12 findings (12 notes).
```

Compliant:

```yaml
name: Release
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    permissions:
      contents: read
    steps:
      - run: make
  publish:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    permissions:
      contents: write
    steps:
      - run: make publish
```

**How to fix.** Declare `permissions` on every job and leave the workflow-level block empty. `--fix` does not rewrite this finding, because the scopes a job needs are a project decision. To ignore a finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#workflow-level-permissions).

## `missing-permissions-block`

Jobs must declare GITHUB_TOKEN permissions instead of relying on defaults.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | pedantic | workflow | none | ghlint: `MissingJobPermissions` |

**What it reports.** Only when the workflow has no top-level `permissions` at all. Every job without its own block is
then reported, including a job that calls a reusable workflow. A workflow-level block of any kind, `read-all` and
`write-all` included, silences the whole rule.

**Why it matters.** The scopes a job without a block gets are resolved outside the workflow, from the repository or
organization defaults, as GitHub's
[`permissions` reference](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#permissions)
describes. The workflow then no longer says what the token can do.

**Example.**

```yaml
name: Token permissions
on: [push]
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: cargo test
```

```text
warning[missing-permissions-block]: job relies on default GITHUB_TOKEN permissions
 --> .github/workflows/example.yml:4:3
  |
4 |   build:
  |   ^^^^^ job `build`
  |
  = help: Jobs must declare GITHUB_TOKEN permissions instead of relying on defaults.
  = note: same as ghlint: MissingJobPermissions

Checked 1 workflow file: 8 findings (1 warning, 7 notes).
```

Compliant:

```yaml
name: Token permissions
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    permissions:
      contents: read
    steps:
      - run: cargo test
```

**How to fix.** Add `permissions` to each job, or one `permissions: {}` at the top when the jobs need no token scope at
all. `--fix` does not rewrite this finding, because it adds no keys that choose a scope. To ignore a finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#missing-permissions-block).

## `secrets-exposure`

Secrets must not be exposed to every step or read in bulk.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | none | zizmor: `overprovisioned-secrets` |

**What it reports.** Two things. Each workflow-level `env` entry whose value reads a named secret is reported, in every
form the expression syntax allows: `secrets.NAME`, `SECRETS.NAME`, `secrets['NAME']`, and `secrets [ 'NAME' ]`. Then,
any string in the file whose expression reads all secrets at once, `toJSON(secrets)`, or indexes `secrets[...]` with a
key that is not a quoted literal, so the name is chosen at run time. Text that only looks like the context is not
reported: a secret name inside a string literal (`${{ 'secrets.NOPE' }}`) or a longer path
(`${{ env.my_secrets.value }}`). A job-level or step-level `env` that holds one named secret is not reported either;
only the workflow level reaches every step.

**Why it matters.** The check follows ghalint 005 (workflow-level secrets in `env`) and zizmor's
`overprovisioned-secrets`
([`src/rules/permissions.rs`](https://github.com/reset-ufc/crosgha/blob/master/src/rules/permissions.rs)). An `env` entry
is a value the runner puts in the environment of every step, and an expression that reads all secrets turns them into
one string that any of those steps can print.

**Example.**

```yaml
name: Secrets
on: [push]
permissions: {}
env:
  DEPLOY_TOKEN: ${{ secrets.DEPLOY_TOKEN }}
jobs:
  deploy:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo '${{ toJSON(secrets) }}' > secrets.json
```

```text
warning[secrets-exposure]: workflow-level env exposes a secret to every step
 --> .github/workflows/example.yml:5:3
  |
5 |   DEPLOY_TOKEN: ${{ secrets.DEPLOY_TOKEN }}
  |   ^^^^^^^^^^^^ workflow
  |
  = help: Secrets must not be exposed to every step or read in bulk.
  = note: same as zizmor: overprovisioned-secrets

warning[secrets-exposure]: expression reads every secret or a secret chosen at runtime
  --> .github/workflows/example.yml:11:9
   |
11 |       - run: echo '${{ toJSON(secrets) }}' > secrets.json
   |         ^^^ workflow
   |
   = help: Secrets must not be exposed to every step or read in bulk.
   = note: same as zizmor: overprovisioned-secrets

Checked 1 workflow file: 8 findings (2 warnings, 6 notes).
```

Compliant:

```yaml
name: Secrets
on: [push]
permissions: {}
jobs:
  deploy:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - env:
          DEPLOY_TOKEN: ${{ secrets.DEPLOY_TOKEN }}
        run: ./deploy
```

**How to fix.** Move the secret to the `env` of the step that needs it, name the secret you read instead of indexing
`secrets` at run time, and pass a value into a script through `env` instead of dumping `secrets` into a file. `--fix` does not rewrite this finding. To ignore a finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#secrets-exposure).

## `secrets-inherit`

Reusable workflow calls should pass only the secrets they need.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | pedantic | workflow | none | zizmor: `secrets-inherit` |

**What it reports.** A job that calls a reusable workflow (`uses`) and sets `secrets: inherit`. A `secrets` mapping
that names each secret is not reported, and neither is a call that passes no secrets.

**Why it matters.** GitHub
[defines](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idsecretsinherit)
`inherit` as passing every secret available to the caller. The called workflow then holds secrets it never uses, and
every one of them reaches another repository's code when the call is remote.

**Example.**

```yaml
name: Reusable workflow secrets
on: [workflow_dispatch]
permissions: { contents: read }
jobs:
  deploy:
    uses: example/workflows/.github/workflows/deploy.yml@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    secrets: inherit
```

```text
note[secrets-inherit]: reusable workflow inherits every available secret
 --> .github/workflows/example.yml:7:5
  |
7 |     secrets: inherit
  |     ^^^^^^^ job `deploy`
  |
  = help: Reusable workflow calls should pass only the secrets they need.
  = note: same as zizmor: secrets-inherit

Checked 1 workflow file: 2 findings (2 notes).
```

Compliant:

```yaml
name: Reusable workflow secrets
on: [workflow_dispatch]
permissions: { contents: read }
jobs:
  deploy:
    uses: example/workflows/.github/workflows/deploy.yml@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    secrets:
      deploy-token: ${{ secrets.DEPLOY_TOKEN }}
```

**How to fix.** Replace `secrets: inherit` with the names the called workflow declares, one line each. `--fix` does not
rewrite this finding. To ignore a finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#secrets-inherit).

## `hardcoded-credentials`

Passwords and access tokens must not be written in workflows.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| error | regular | workflow | none | zizmor: `hardcoded-container-credentials`; actionlint: `credentials` |

**What it reports.** Two things. A `credentials.password` of a job's `container` or of one of its `services` is
reported when it holds no `${{` expression. It also reports any string in the file that holds a literal token:
`ghp_`, `gho_`, `ghu_`, `ghs_`, or `ghr_` followed by at least 36 alphanumerics, `github_pat_` followed by at least 22
alphanumerics or underscores, `AKIA` followed by 16 uppercase letters or digits, or both `-----BEGIN` and
`PRIVATE KEY-----`. A prefix counts only at the start of the value or after a character that is not an ASCII letter
or digit, so a longer identifier that only ends like a token is not reported. Container `username` values are not checked, and a password written in another form, without one of those
prefixes, is not reported.

**Why it matters.** Those are the token formats GitHub and AWS
[publish](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/about-authentication-to-github#githubs-token-formats)
for secret scanning; the AWS prefixes are listed in the
[unique identifier reference](https://docs.aws.amazon.com/IAM/latest/UserGuide/reference_identifiers.html#identifiers-unique-ids).
A token written in a workflow is readable by everyone who can read the repository, and it stays in the history after
the line is removed.

**Example.**

```yaml
name: Credentials
on: [push]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    container:
      image: ghcr.io/example/app:1
      credentials:
        username: ci
        password: hunter2
    steps:
      - run: |
          curl -H "Authorization: token ghp_000000000000000000000000000000000000" https://api.github.com
```

```text
error[hardcoded-credentials]: container registry password is written in the workflow
  --> .github/workflows/example.yml:12:9
   |
12 |         password: hunter2
   |         ^^^^^^^^ workflow
   |
   = help: Passwords and access tokens must not be written in workflows.
   = note: same as zizmor: hardcoded-container-credentials; actionlint: credentials

error[hardcoded-credentials]: file contains a literal access token
  --> .github/workflows/example.yml:14:9
   |
14 |       - run: |
   |         ^^^ workflow
   |
   = help: Passwords and access tokens must not be written in workflows.
   = note: same as zizmor: hardcoded-container-credentials; actionlint: credentials

Checked 1 workflow file: 8 findings (2 errors, 1 warning, 5 notes).
```

Compliant:

```yaml
name: Credentials
on: [push]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    container:
      image: ghcr.io/example/app:1
      credentials:
        username: ci
        password: ${{ secrets.REGISTRY_PASSWORD }}
    steps:
      - env:
          TOKEN: ${{ secrets.API_TOKEN }}
        run: |
          curl -H "Authorization: token $TOKEN" https://api.github.com
```

**How to fix.** Read the password or token from a secret and reference the environment variable in the script. Revoke
the token that was written in the workflow: removing the line leaves it in the repository history. `--fix` does not rewrite this finding. To ignore a finding, see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#hardcoded-credentials).

## `persist-credentials`

actions/checkout must not leave the token on disk for later steps; there is no automatic fix when a later step pushes
with it, `true` is explicit, or the checkout predates v2.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | [`--fix`](../fixes.md) | zizmor: `artipacked` |

**What it reports.** An `actions/checkout` step, matched without case, whose `with` omits `persist-credentials` (the
input defaults to `true`) or sets it to a true value, written as `true` or as the string `TRUE` in any case. A `false`
value is not reported, and neither is a value that is an expression. The finding also names the reason a fix is not
offered, when one applies: `persist-credentials: true` written out, a later step of the same job pushes with the token
(`git push`, or `stefanzweifel/git-auto-commit-action`), the ref predates v2, which has no such input, or the step sits
in an action file, whose later steps belong to the caller. A push from another job is not considered.

**Why it matters.** `actions/checkout`
[documents](https://github.com/actions/checkout/blob/main/README.md#usage) the true default and the `false` opt-out, and
says the persisted token "enables your scripts to run authenticated git commands". Every later step of the job, and
anything it runs, can then use the token it finds on disk.

**Example.**

```yaml
name: Checkout credentials
on: [push]
permissions: { contents: read }
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Checkout
        uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
      - name: Test
        run: cargo test
```

```text
warning[persist-credentials]: checkout retains credentials for later steps
  --> .github/workflows/example.yml:10:9
   |
10 |         uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
   |         ^^^^ job `test`, step 1
   |
   = help: actions/checkout must not leave the token on disk for later steps; there is no automatic fix when a later step pushes with it, `true` is explicit, or the checkout predates v2.
   = note: same as zizmor: artipacked

Checked 1 workflow file: 8 findings (2 warnings, 6 notes).
1 finding has a fix: rerun with --fix.
```

With `--fix`, the step gets the input and the finding goes away:

```text
$ crosgha --offline --persona auditor --fix .github/workflows/example.yml
Fixed 1 finding in 1 workflow file.
Remaining: 7 findings (1 warning, 6 notes).
```

The rewritten step:

```yaml
      - name: Checkout
        uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
        with:
          persist-credentials: false
```

Compliant:

```yaml
name: Checkout credentials
on: [push]
permissions: { contents: read }
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Checkout
        uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
        with:
          persist-credentials: false
      - name: Test
        run: cargo test
```

**How to fix.** Add `persist-credentials: false` under the step's `with` (creating `with` if needed).
`--fix` adds it after [verifying](../fixes.md) that the file still parses, the finding is gone, and only
the intended node changed. It leaves the finding unfixed, with the reason in the message, in four cases: `true` is
explicit, a later step of the job pushes with the persisted token, the checkout predates v2, or the step is in an action
file. Give that pushing step its own token (`${{ github.token }}` in its `env`) and drop the persisted one. To ignore a
finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#persist-credentials).

## `oidc-misconfig`

`id-token: write` must be scoped to the job that needs it.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | pedantic | workflow | none | none |

**What it reports.** A workflow-level `id-token: write` is reported when more than one job inherits it ("every job can
mint an OIDC token") or when any trigger outsiders control is present ("OIDC token is mintable on a trigger outsiders
control"). A job-level `id-token: write` is reported only when one of those triggers is present. The triggers are
`fork`, `watch`, `issues`, `issue_comment`, `discussion`, `discussion_comment`, `pull_request_target`,
`pull_request_review`, `pull_request_review_comment`, and `workflow_run`. Any other scope, and `id-token: read`, is not
reported.

**Why it matters.** GitHub
[describes](https://docs.github.com/en/actions/concepts/security/openid-connect) the OIDC token as short-lived
credentials a job exchanges for a cloud role. A job that can mint one on a trigger an outsider can start lets that outsider's
event request the token; what it can then reach depends on the cloud role's trust policy.

**Example.**

```yaml
name: OIDC
on: [push]
permissions:
  contents: read
  id-token: write
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo test
  deploy:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo deploy
```

```text
warning[oidc-misconfig]: every job can mint an OIDC token
 --> .github/workflows/example.yml:5:3
  |
5 |   id-token: write
  |   ^^^^^^^^ workflow
  |
  = help: `id-token: write` must be scoped to the job that needs it.

Checked 1 workflow file: 15 findings (2 warnings, 13 notes).
```

Compliant:

```yaml
name: OIDC
on: [push]
permissions:
  contents: read
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: echo test
  deploy:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    permissions:
      contents: read
      id-token: write
    steps:
      - run: echo deploy
```

**How to fix.** Move `id-token: write` into the `permissions` of the job that exchanges the token, usually the one that
deploys, and keep it out of every workflow that runs on a trigger outsiders control. `--fix` does not rewrite this
finding. To ignore a finding, see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#oidc-misconfig).
