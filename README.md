# crosgha

aCtions Repair Of miSconfigured GitHub Actions: a Rust CLI and GitHub Action that detects smells in GitHub Actions
workflows and fixes them deterministically without changing comments or formatting.

- 51 rules cover the smell taxonomy: 43 check workflow or action files, 3 check the repository, and 5 query the GitHub
  API.
- Composite actions (`action.yml`) get the step rules, and JavaScript and Docker actions their metadata checks: the
  repository's root action and the local actions its workflows call are found with the workflows. With
  `--follow-actions`, so are the actions they call from other repositories, read at the pinned commit and followed
  through composite actions; each finding names the chain of `uses:` lines that pulled it in.
- It reads `zizmor.yml`, accepts zizmor's rule names and ignore comments, and names the equivalent zizmor, actionlint, or ghlint
  rule when there is one, so switching tools does not mean rewriting configuration.
- Output is text, JSON, SARIF 2.1.0, GitHub annotations (default under `GITHUB_ACTIONS=true`), or brief lines for AI
  agents.

License: [Apache-2.0](LICENSE).

## Install

On Linux x86_64, this installs the latest release to `~/.local/bin` after checking it against the release's
`sha256.sum`:

```sh
curl -fsSL https://raw.githubusercontent.com/reset-ufc/crosgha/master/install.sh | sh
```

Release archives and building from source: [docs/install.md](docs/install.md).

## Quick start

```sh
crosgha                                   # scans .github/workflows and the actions it calls
crosgha path/to/workflow.yml
crosgha path/to/action.yml                # a composite, JavaScript, or Docker action
crosgha --persona pedantic .github/workflows
crosgha --fix .github/workflows
crosgha -f sarif > results.sarif          # also: json, github, agent
crosgha --sarif-file results.sarif .      # text on stdout, SARIF on disk
crosgha --follow-actions                  # also check called actions of other repositories (needs a token)
crosgha --list-rules                      # add -f json for summaries and equivalents
```

## GitHub Action

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

Pin the Action by full commit SHA with a version comment; there is no moving `v1` tag. Inputs, outputs, and how a run
goes: [docs/action.md](docs/action.md).

## Documentation

The documentation is published at https://reset-ufc.github.io/crosgha/.

| Document | Covers |
|---|---|
| [docs/getting-started.md](docs/getting-started.md) | A walkthrough: first scan, personas, `--fix`, CI, and the pre-push hook |
| [docs/how-it-works.md](docs/how-it-works.md) | What is scanned, rule scopes, personas, how a fix is accepted, and the output formats |
| [docs/install.md](docs/install.md) | Install script options, release archives, building from source |
| [docs/usage.md](docs/usage.md) | Which paths and actions are scanned, the pre-push hook, migrating from zizmor, actionlint, poutine, ghalint, or octoscan |
| [docs/action.md](docs/action.md) | Action inputs, outputs, and run steps |
| [docs/configuration.md](docs/configuration.md) | Personas, online rules, `crosgha.yml` / `zizmor.yml`, ignoring findings, severity |
| [docs/output.md](docs/output.md) | Output formats and exit status |
| [docs/fixes.md](docs/fixes.md) | When `--fix` applies a fix, and which rules have one |
| [docs/rules.md](docs/rules.md) | The rule catalog: tag, severity, persona, scope, equivalents, examples |
| [docs/rules/](docs/rules/supply-chain.md) | Each rule explained by dimension, with an example, its output, and how to fix it |
| [docs/github-guidance.md](docs/github-guidance.md) | GitHub's own Actions guidance mapped to the rule that checks each statement |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Module interfaces, the rule contract, and fix safety |
| [docs/rust-style.md](docs/rust-style.md) | The Rust style guide for contributors |

## Development

```sh
make setup   # enables the versioned git hooks in .githooks/; trusts mise.toml only if mise is installed
cargo test   # end-to-end and fixture-driven rule tests
cargo fmt-style   # format Rust per docs/rust-style.md; plain cargo fmt skips its import rules
make check   # the pre-push checks: fmt, clippy -D warnings, tests, dogfood scan
```

- The pre-push hook runs under `mise exec` when mise is not active in the calling shell, so pushes from `gh` or an IDE
  use the pinned toolchain too.
- Commits follow Conventional Commits (enforced by the `commit-msg` hook). release-please turns them into versions and
  the changelog, and cargo-dist builds the release binaries.
