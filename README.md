# our-tool

Placeholder name for a future tool that detects and auto-fixes smells in GitHub Actions
workflows. This repository currently ships only the project skeleton and release toolchain;
no smell-detection logic exists yet.

License: TBD by the author.

## Build

```sh
cargo build --release
```

## Run

```sh
./target/release/our-tool          # hello from our-tool
./target/release/our-tool --version
```

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
