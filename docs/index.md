# crosgha

aCtions Repair Of miSconfigured GitHub Actions: a Rust CLI and GitHub Action that detects smells in GitHub Actions
workflows and fixes them deterministically, preserving comments and formatting.

- 51 rules cover every tag of a curated smell taxonomy: 43 check workflow or action files, 3 check the repository,
  and 5 query the GitHub API.
- Output is text, JSON, SARIF 2.1.0, GitHub annotations (the default under `GITHUB_ACTIONS=true`), or brief lines for
  AI agents.
- It reads `zizmor.yml`, accepts zizmor's rule names and ignore comments, and names the equivalent zizmor or
  actionlint rule in every finding.

## Install

On Linux x86_64, this installs the latest release to `~/.local/bin` after checking it against the release's
`sha256.sum`:

```sh
curl -fsSL https://raw.githubusercontent.com/reset-ufc/crosgha/master/install.sh | sh
```

Release archives and building from source: [Install](install.md).

## First scan

```sh
crosgha                              # scans .github/workflows and the actions it calls
crosgha --persona pedantic .         # also hardening policies: pinning, timeouts, permissions
crosgha --fix .github/workflows      # apply the fixes that pass validation
crosgha -f sarif > results.sarif     # also: json, github, agent
```

Without a `PATH`, crosgha scans `.github/workflows`. It reports findings as rustc-style diagnostics and exits 1 when a
finding reaches `--fail-on`, which defaults to `any`. Which paths and actions are scanned: [Usage](usage.md). How to
read the output and the exit status: [Output and exit status](output.md).

## In a workflow

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

Pin the Action by full commit SHA with a version comment; there is no moving `v1` tag. It can also open a pull
request with the fixes ([GitHub Action](action.md)).

## Documentation

| Page | Covers |
|---|---|
| [Getting started](getting-started.md) | A walkthrough: first scan, personas, `--fix`, the remaining fixes by hand, CI, and the hook |
| [How crosgha works](how-it-works.md) | What is scanned, rule scopes, personas, how a fix is accepted, and the output formats |
| [Install](install.md) | Install script options, release archives, building from source |
| [Usage](usage.md) | Which paths and actions are scanned, remote repositories, the pre-push hook, migrating from other tools |
| [GitHub Action](action.md) | Action inputs, outputs, the fix pull request, and auto-merge |
| [Configuration](configuration.md) | Personas, online rules, `crosgha.yml` / `zizmor.yml`, ignoring findings, severity |
| [Output and exit status](output.md) | Output formats and exit status |
| [Fixes](fixes.md) | When `--fix` applies a fix, and which rules have one |
| [Rules](rules.md) | The rule catalog: tag, severity, persona, scope, equivalents |
| [Rules by dimension](rules/supply-chain.md) | Each rule explained with an example, its output, and how to fix it: [supply chain](rules/supply-chain.md), [injection](rules/injection.md), [permissions and secrets](rules/permissions-secrets.md), [runners](rules/runner.md), [maintainability](rules/maintainability.md), [correctness](rules/correctness.md), [repository](rules/repository.md) |
| [Rule examples](rule-examples.md) | Non-compliant and compliant YAML for every rule, generated from the tests |
| [What GitHub's documentation asks](github-guidance.md) | GitHub's normative statements mapped to the rule that checks each |
| [Architecture](ARCHITECTURE.md) | Module interfaces, the rule contract, and fix safety |
| [Rust style](rust-style.md) | The Rust style guide for contributors |

License: [Apache-2.0](https://github.com/reset-ufc/crosgha/blob/master/LICENSE).