# GitHub Action

```yaml
permissions:
  contents: read
  security-events: write # only for the SARIF upload (advanced-security: true, the default)
steps:
  - uses: actions/checkout@<sha> # vX.Y.Z
    with:
      persist-credentials: false
  - uses: reset-ufc/crosgha@<sha> # vX.Y.Z
    with:
      persona: pedantic
```

Pin the Action by full commit SHA with a version comment; there is no moving `v1` tag. It takes zizmor-action's inputs
(`inputs`, `persona`, `min-severity`, `online-audits`, `token`, `advanced-security`, `annotations`, `config`,
`fail-on-no-inputs`, and more) plus `fail-on` and `follow-actions` (default `false`; checks the actions that steps
call from other repositories, needs `online-audits` and a token, and spends API requests: see
[usage](usage.md#actions-from-other-repositories)), and sets the outputs `output-file` (SARIF path) and `findings` (count).
A run goes like this:

1. It downloads the release that matches its own revision and checks the archive against the release's `sha256.sum`
   before running it.
2. It runs crosgha once. The log and the job summary get the uncolored text report; `color` is accepted for
   zizmor-action compatibility and changes nothing. With `annotations: true`, the log gets annotations and the job
   summary only the finding count.
3. It uploads SARIF with `github/codeql-action/upload-sarif`, pinned by SHA.
4. Only then does it fail according to `fail-on`.

Linux X64 runners only.

## Fix pull requests

With `fix: pull-request`, after the report the Action runs `crosgha --fix` on the same inputs, commits only the files
it wrote to `crosgha/fix-<run_id>`, and opens a pull request against the branch the run checked out. The body lists
each fix with its file, line, and rule. A human reviews and merges it; the Action never merges. The caller's checkout
keeps its HEAD, index, and other changes, and `fail-on` still applies to the findings of the scanned branch. The
output `pull-request-url` names the pull request, and stays empty when no fix applied.

Every fix touches `.github/workflows/`, which `GITHUB_TOKEN` cannot write: it has no `workflows` permission. Pass
`fix-token`, a GitHub App installation token (recommended) or a fine-grained personal access token, with Contents,
Pull requests, and Workflows: write. The push and the pull request use only `fix-token`; `token` stays the token of
the online rules. The run must check out a branch (push, schedule, or `workflow_dispatch`, not `pull_request`), and the
runner needs `git` and `jq`.

```yaml
on:
  schedule:
    - cron: "0 6 * * 1"
permissions:
  contents: read
  security-events: write
jobs:
  crosgha:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - id: app
        uses: actions/create-github-app-token@64ab729a6294000f5aa4a4bc0b73d1f32bc45510 # v3.3.0
        with:
          client-id: ${{ vars.CROSGHA_APP_CLIENT_ID }}
          private-key: ${{ secrets.CROSGHA_APP_PRIVATE_KEY }}
          permission-contents: write
          permission-pull-requests: write
          permission-workflows: write
      - uses: actions/checkout@<sha> # vX.Y.Z
        with:
          persist-credentials: false
      - uses: reset-ufc/crosgha@<sha> # vX.Y.Z
        with:
          fix: pull-request
          fix-token: ${{ steps.app.outputs.token }}
```

When an earlier step of the job already changed a file that crosgha then fixes, the Action fails instead of
committing that change with the fix. A re-run of the same workflow run finds `crosgha/fix-<run_id>` already pushed and fails; delete the branch or start a
new run.

## Auto-merge

`dangerously-auto-merge: true` (with `fix: pull-request`) enables GitHub
[auto-merge](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/automatically-merging-a-pull-request)
on the pull request it opened, through the GraphQL `enablePullRequestAutoMerge` mutation, so GitHub merges it once
its required reviews and status checks pass. The Action never merges by itself: a merged fix changes your workflows
without a human reading it, at your own risk.

- Before it pushes anything, the Action reads the base branch's rules and classic branch protection with `fix-token`.
  When neither requires a status check, or it cannot read them, it fails instead of opening a pull request that
  would merge unchecked.
- Auto-merge must be allowed in the repository settings, and the pull request uses the repository's default merge
  method. When GitHub refuses to enable it, the step fails and names the pull request it left open.
- Every run with the flag puts a warning in the job summary.
