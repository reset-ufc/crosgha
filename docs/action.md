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
`fail-on-no-inputs`, and more) plus `fail-on`, and sets the outputs `output-file` (SARIF path) and `findings` (count).
A run goes like this:

1. It downloads the release that matches its own revision and checks the archive against the release's `sha256.sum`
   before running it.
2. It runs crosgha once. The log and the job summary get the text report.
3. It uploads SARIF with `github/codeql-action/upload-sarif`, pinned by SHA.
4. Only then does it fail according to `fail-on`.

Linux X64 runners only. `fix: pull-request` is validated but not implemented yet.
