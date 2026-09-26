# our-tool

Placeholder name for a tool that will detect and auto-fix smells in GitHub Actions workflows.
The current Rust scaffold recursively scans workflow files with `clap`, parses them with
`yamlpath`, and verifies the top-level `on` and `jobs` structure. No smell rules or fixes are
implemented yet; `yamlpatch` is included for the upcoming fix layer.

License: TBD by the author.

## Build

```sh
cargo build --release
```

## Run

```sh
./target/release/our-tool          # scans .github/workflows
./target/release/our-tool --version
./target/release/our-tool path/to/workflow.yml
```

`PATH` may be one workflow file or a directory. Directories are scanned recursively.

## Test

```sh
cargo test
```

## Release

Versioning and releases are automated. See [RELEASING.md](RELEASING.md).

## Development setup

```sh
make setup   # enables the versioned git hooks in .githooks/
make check   # fmt --check, clippy -D warnings, cargo test
```
