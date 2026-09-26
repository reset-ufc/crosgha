# crosgha

aCtions Repair Of miSconfigured GitHub Actions: a tool that detects and fixes smells in GitHub Actions workflows.
The CLI recursively scans workflow files, parses YAML with `yamlpath`, and verifies the top-level
`on` and `jobs` structure. It reports `actions/checkout` steps that retain credentials by default
or explicitly set `persist-credentials: true`. Four additional local policy checks are opt-in.

License: TBD by the author.

## Build

```sh
cargo build --release
```

## Run

```sh
./target/release/crosgha          # scans .github/workflows
./target/release/crosgha --version
./target/release/crosgha path/to/workflow.yml
./target/release/crosgha --fix path/to/workflow.yml
./target/release/crosgha --format text path/to/workflow.yml
./target/release/crosgha --fail-on error path/to/workflow.yml
./target/release/crosgha --require-immutable-references --require-job-timeout \
  --require-explicit-token-permissions --require-explicit-workflow-secrets .github/workflows
```

`PATH` may be one workflow file or a directory. Directories are scanned recursively.
By default, scanning exits with status 1 when it finds a smell. The four policy options are off by
default and can be combined independently:

| Option | Rule | Scope |
|---|---|---|
| `--require-immutable-references` | `mutable-action-reference` | Literal remote action and reusable workflow refs need a 40-character hexadecimal commit SHA; literal step, job, and service images need a SHA-256 or SHA-512 digest. Local refs are exempt. |
| `--require-job-timeout` | `missing-job-timeout` | Jobs with `runs-on` need `timeout-minutes`; reusable workflow calls are excluded. This checks presence, not a numeric ceiling. |
| `--require-explicit-token-permissions` | `implicit-token-permissions` | Each job needs job-level `permissions` unless the workflow sets top-level `permissions`. |
| `--require-explicit-workflow-secrets` | `inherited-workflow-secrets` | Reusable workflow calls cannot use `secrets: inherit`. This is a scope signal, not evidence of a leak. |

The reference check validates literal pin syntax. It does not resolve action commits, verify their
repository identity, or query image registries. Dynamic expressions are skipped because their
resolved values are unavailable during a static scan. The reference, timeout, permissions, and
secrets rules report findings without proposing replacements: choosing a commit, timeout, token
scope, or secret selection requires project context.

`--fix` sets `persist-credentials: false` for supported checkout steps. It verifies that patched
YAML parses, the targeted finding disappears, no new finding appears, and only the intended YAML
node changed before writing. Explicit expressions for `persist-credentials` are left untouched
because their value is not known at scan time. Findings that cannot be fixed are listed after the
fix count. Review a fix before relying on authenticated Git commands later in the job.
`--fail-on` defaults to `any`; `note`, `warning`, `error`, and `none` are also
accepted. Checkout credentials, mutable references, and implicit token permissions are `warning`;
missing timeouts and inherited secrets are `note`. Findings are printed even when the threshold does
not make the command fail.

SARIF output and GitHub Action packaging are planned in [ROADMAP.md](ROADMAP.md).
The module interfaces and Action flow are in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
`--format json` and `--format sarif` currently fail before any file is changed.
The root `action.yml` is a nonfunctional scaffold: it validates runner and mode inputs, then
fails before downloading a binary or changing a repository.

## Test

```sh
cargo test
```

## Release

Versioning and releases are automated by release-please (from Conventional Commits) and cargo-dist.

## Development setup

```sh
make setup   # enables the versioned git hooks in .githooks/
make check   # fmt --check, clippy -D warnings, cargo test
```
