# Repository

These two rules read the repository around the workflows: which security tooling it runs, from the files next to the
workflows, and how its Actions settings are configured, from the GitHub API.

## `missing-security-tooling`

Repositories should run dependency updates, CodeQL, dependency review, and OpenSSF Scorecard.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | auditor | repository | none | none |

**What it reports.** Four separate findings, one per missing piece. The first is reported when none of the eight update
configs exists: `.github/dependabot.yml`, `.github/dependabot.yaml`, `.github/renovate.json`,
`.github/renovate.json5`, `renovate.json`, `renovate.json5`, `.renovaterc`, `.renovaterc.json`. The other three are
reported when the text of the `.github/workflows/*.yml` and `*.yaml` files, read without case, holds no
`github/codeql-action/`, no `actions/dependency-review-action`, and no `ossf/scorecard-action`. Text is matched, not
YAML: a mention in a comment counts, and a workflow that calls one of those actions through another action is not
recognized. Apart from the update configs, it reads nothing outside `.github/workflows`, and inline ignore comments and
the config `ignore` entries do not apply to it, because its findings belong to the repository, not to a file or a line. Disable it with
`rules.missing-security-tooling.disable` ([configuration](../configuration.md#config-file)).

**Why it matters.** The rule follows Fares et al. (arXiv
[2601.14455](https://arxiv.org/abs/2601.14455)), hardening gap FGB-05, and the StepSecurity secure-repo checks,
both cited in the rule's [code](https://github.com/reset-ufc/crosgha/blob/master/src/rules/repository.rs). Each of the
four checks catches a class of supply chain problem before it reaches a commit: a vulnerable dependency, a code
pattern, a new dependency added in a pull request, and the repository's own posture.

**Example.** The fixture is a repository with one workflow and no update config:

```
.github/
└── workflows/
    └── ci.yml
```

```yaml
name: CI
on: [push]
permissions: {}
jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Test
        run: make test
```

Run on the fixture repository with `--offline`:

```text
note[missing-security-tooling]: repository has no automated dependency updates (Dependabot or Renovate)
 --> ./.github:1:1
  = at: repository
  = help: Repositories should run dependency updates, CodeQL, dependency review, and OpenSSF Scorecard.

note[missing-security-tooling]: repository has no CodeQL workflow
 --> ./.github:1:1
  = at: repository
  = help: Repositories should run dependency updates, CodeQL, dependency review, and OpenSSF Scorecard.

note[missing-security-tooling]: repository has no dependency review workflow
 --> ./.github:1:1
  = at: repository
  = help: Repositories should run dependency updates, CodeQL, dependency review, and OpenSSF Scorecard.

note[missing-security-tooling]: repository has no OpenSSF Scorecard workflow
 --> ./.github:1:1
  = at: repository
  = help: Repositories should run dependency updates, CodeQL, dependency review, and OpenSSF Scorecard.

Checked 1 workflow file: 10 findings (10 notes).
```

Every finding points at the repository as a whole, so the location is the `.github` directory, at line 1.

Compliant: the [hardened fixture](https://github.com/reset-ufc/crosgha/tree/master/tests/fixtures/repos/hardened)
holds an update config and one workflow that runs the three scanners:

```
.github/
├── dependabot.yml
└── workflows/
    └── security.yml
```

That repository reports no `missing-security-tooling` finding.

**How to fix.** Add `.github/dependabot.yml` with one `updates` entry per ecosystem, and a workflow that runs
`github/codeql-action/init` and `github/codeql-action/analyze`, `actions/dependency-review-action` on pull requests, and
`ossf/scorecard-action`. `--fix` does not rewrite this finding: it creates no files, as stated in [fixes](../fixes.md).

All fixtures for this rule: [examples](../rule-examples.md#missing-security-tooling).

## `repo-org-settings`

Repository Actions settings must not grant write tokens or pull request approval by default.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | pedantic | repository | none | none |

**What it reports.** It reads `/repos/{owner}/{repo}/actions/permissions/workflow`, the
[default workflow permissions](https://docs.github.com/en/rest/actions/permissions#get-default-workflow-permissions-for-a-repository)
of the repository, and reports each of two settings: `default_workflow_permissions` equal to `write`, and
`can_approve_pull_request_reviews` equal to `true`. Both findings point at the repository, not at a workflow. It reads
one set of settings per scan, so it says nothing about the organization's own settings.

**Why it matters.** A `write` default gives every job of every workflow that declares no `permissions` write access to
the repository. `can_approve_pull_request_reviews` lets `GITHUB_TOKEN` approve pull requests, so a workflow can satisfy
an approval that a person was meant to give.

The rule queries the GitHub API, so `--offline` skips it silently, and so does a scan with no token. It needs a token
with the Administration (read) repository permission or a classic token's `repo` scope, which `GITHUB_TOKEN` cannot be
granted, so under Actions it is skipped with a warning; a local input also needs `--repository owner/name`, and a
remote input checks its own repository ([configuration](../configuration.md#online-rules)).

**Example.** The [fixture](https://github.com/reset-ufc/crosgha/tree/master/tests/fixtures/repos/repo-org-settings) is a
repository whose workflow declares no token scopes. The rule does not read the workflow; it reads the API response
served from [`tests/fixtures/online/api.json`](https://github.com/reset-ufc/crosgha/blob/master/tests/fixtures/online/api.json):

```json
{
  "default_workflow_permissions": "write",
  "can_approve_pull_request_reviews": false
}
```

Compliant:

```json
{
  "default_workflow_permissions": "read",
  "can_approve_pull_request_reviews": false
}
```

**How to fix.** In the repository's Actions settings (Settings, Actions, General), set the default workflow
permissions to `read` and leave the pull request approval setting off, which is what
`can_approve_pull_request_reviews: false` means. `--fix` does not rewrite this finding: the setting lives on GitHub,
not in a file ([fixes](../fixes.md)). To accept a finding, ignore it as described in
[configuration](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#repo-org-settings).
