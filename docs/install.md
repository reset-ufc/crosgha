# Install

## Install script

On Linux x86_64, this installs the latest release to `~/.local/bin` after checking it against the release's
`sha256.sum`:

```sh
curl -fsSL https://crosgha.resetlab.com.br/install.sh | sh
```

`CROSGHA_VERSION=X.Y.Z` pins a release and `CROSGHA_INSTALL_DIR` changes the target directory. Uninstall with
`rm ~/.local/bin/crosgha`.

## From a release

Releases ship one statically linked Linux binary (`x86_64-unknown-linux-musl`), so it also runs in Alpine
containers, with a `sha256.sum` and a build provenance attestation:

```sh
gh release download -R reset-ufc/crosgha -p crosgha-x86_64-unknown-linux-musl.tar.xz -p sha256.sum
sha256sum -c sha256.sum
tar -xJf crosgha-x86_64-unknown-linux-musl.tar.xz
install crosgha-x86_64-unknown-linux-musl/crosgha ~/.local/bin/
```

`gh attestation verify crosgha-x86_64-unknown-linux-musl.tar.xz -R reset-ufc/crosgha` checks that the archive was
built by this repository's release workflow.

## From source

Needs Rust 1.97, pinned in `mise.toml`:

```sh
mise install
cargo build --release   # binary at target/release/crosgha
```
