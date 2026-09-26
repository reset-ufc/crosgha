# crosgha

aCtions Repair Of miSconfigured GitHub Actions: a tool that detects and fixes smells in GitHub Actions workflows.
The CLI recursively scans workflow files, parses YAML with `yamlpath`, and verifies the top-level
`on` and `jobs` structure. Its first rule reports `actions/checkout` steps that retain credentials
by default or explicitly set `persist-credentials: true`.

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
```

`PATH` may be one workflow file or a directory. Directories are scanned recursively.
By default, scanning exits with status 1 when it finds a smell. `--fix` sets `persist-credentials: false` for
supported checkout steps. It verifies that patched YAML parses, the targeted finding disappears,
no new finding appears, and only the intended YAML node changed before writing. Explicit
expressions for `persist-credentials` are left untouched because their value is not known at scan
time. Review a fix before relying on authenticated Git commands later in the job.
`--fail-on` defaults to `any`; `note`, `warning`, `error`, and `none` are also
accepted. The implemented rule is classified as `warning` by the current
project policy. Findings are printed even when the selected threshold does
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
