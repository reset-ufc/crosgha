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

Releases ship one Linux binary (`x86_64-unknown-linux-gnu`) with a `sha256.sum`:

```sh
gh release download -R reset-ufc/crosgha -p crosgha-x86_64-unknown-linux-gnu.tar.xz -p sha256.sum
sha256sum -c sha256.sum
tar -xJf crosgha-x86_64-unknown-linux-gnu.tar.xz
install crosgha-x86_64-unknown-linux-gnu/crosgha ~/.local/bin/
```

## From source

Needs Rust 1.97, pinned in `mise.toml`:

```sh
mise install
cargo build --release   # binary at target/release/crosgha
```
