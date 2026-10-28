# crosgha

aCtions Repair Of miSconfigured GitHub Actions: a Rust CLI and GitHub Action that detects smells in GitHub Actions
workflows and fixes them deterministically, preserving comments and formatting.

- 48 rules cover every tag of a curated smell taxonomy: 39 check workflow files, 4 check the repository, and 5 query
  the GitHub API.
- It reads `zizmor.yml`, accepts zizmor's rule names and ignore comments, and names the equivalent zizmor or
  actionlint rule in every finding, so switching tools does not mean rewriting configuration.
- Output is text, JSON, SARIF 2.1.0, GitHub annotations (default under `GITHUB_ACTIONS=true`), or brief lines for AI
  agents.

License: TBD by the author.

## Install

On Linux x86_64, this installs the latest release to `~/.local/bin` after checking it against the release's
`sha256.sum`:

```sh
curl -fsSL https://crosgha.resetlab.com.br/install.sh | sh
```

Release archives and building from source: [docs/install.md](docs/install.md).

## Quick start

```sh
crosgha                                   # scans .github/workflows
crosgha path/to/workflow.yml
crosgha --persona pedantic .github/workflows
crosgha --fix .github/workflows
crosgha -f sarif > results.sarif          # also: json, github, agent
crosgha --sarif-file results.sarif .      # text on stdout, SARIF on disk
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

| Document | Covers |
|---|---|
| [docs/install.md](docs/install.md) | Install script options, release archives, building from source |
| [docs/usage.md](docs/usage.md) | Which paths are scanned, the pre-push hook, migrating from zizmor or actionlint |
| [docs/action.md](docs/action.md) | Action inputs, outputs, and run steps |
| [docs/configuration.md](docs/configuration.md) | Personas, online rules, `crosgha.yml` / `zizmor.yml`, ignoring findings, severity |
| [docs/output.md](docs/output.md) | Output formats and exit status |
| [docs/fixes.md](docs/fixes.md) | When `--fix` applies a fix, and which rules have one |
| [docs/rules.md](docs/rules.md) | The rule catalog: tag, severity, persona, scope, equivalents |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Module interfaces, the rule contract, and fix safety |
| [ROADMAP.md](ROADMAP.md) | What is shipped and what is planned |

## Development

```sh
make setup   # enables the versioned git hooks in .githooks/; trusts mise.toml only if mise is installed
cargo test   # end-to-end and fixture-driven rule tests
make check   # the pre-push checks: fmt, clippy -D warnings, tests, dogfood scan
```

- **Toolchain:** the pre-push hook runs under `mise exec` when mise is not active in the calling shell, so pushes from
  `gh` or an IDE use the pinned toolchain too.
- **Releases:** commits follow Conventional Commits (enforced by the `commit-msg` hook). release-please turns them
  into versions and the changelog, and cargo-dist builds the release binaries.
