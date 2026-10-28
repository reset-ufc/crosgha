#!/bin/sh
# Installs the crosgha release binary for the current user:
#   curl -fsSL https://crosgha.resetlab.com.br/install.sh | sh
# CROSGHA_VERSION pins a release (default: latest); CROSGHA_INSTALL_DIR overrides ~/.local/bin.
# Everything runs inside main, so a download cut short never executes half a script.
set -eu

fail() {
    printf 'crosgha install: %s\n' "$1" >&2
    exit 1
}

main() {
    platform="$(uname -s) $(uname -m)"
    # ponytail: releases ship Linux x86_64 only; add a case per target once dist-workspace.toml builds it.
    case "$platform" in
        "Linux x86_64") target=x86_64-unknown-linux-musl ;;
        *) fail "no release binary for $platform; build from source: cargo install --git https://github.com/reset-ufc/crosgha" ;;
    esac
    for tool in curl tar xz sha256sum; do
        command -v "$tool" >/dev/null 2>&1 || fail "$tool is required"
    done

    base="${CROSGHA_RELEASE_BASE:-https://github.com/reset-ufc/crosgha/releases}"
    protocol='=https'
    # Tests serve a local release over plain HTTP; nothing else may.
    case "$base" in
        http://127.0.0.1:|http://127.0.0.1:*[!0-9]*) ;;
        http://127.0.0.1:*) protocol='=http' ;;
    esac
    version="${CROSGHA_VERSION:-}"
    if [ -n "$version" ]; then
        version="v${version#v}"
        url="$base/download/$version"
    else
        version=latest
        url="$base/latest/download"
    fi

    asset="crosgha-$target.tar.xz"
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    for file in "$asset" sha256.sum; do
        curl --proto "$protocol" --tlsv1.2 -fsSL --retry 3 -o "$tmp/$file" "$url/$file" \
            || fail "cannot download $file for crosgha $version"
    done
    # Releases up to v0.9.0 write `*name` (binary mode). A missing line leaves sha256sum with no
    # input, which fails like a mismatch.
    (cd "$tmp" && awk -v asset="$asset" '$2 == asset || $2 == "*" asset' sha256.sum \
        | sha256sum --check --status -) \
        || fail "checksum mismatch for $asset; refusing to install it"
    tar -xJf "$tmp/$asset" -C "$tmp"

    dir="${CROSGHA_INSTALL_DIR:-$HOME/.local/bin}"
    mkdir -p "$dir"
    # Copy next to the target, then rename, so a running crosgha is never overwritten mid-write.
    cp "$tmp/crosgha-$target/crosgha" "$dir/.crosgha.new"
    chmod 755 "$dir/.crosgha.new"
    # A binary for another libc or CPU passes the checksum; keep the working one unless this runs.
    if ! installed="$("$dir/.crosgha.new" --version)"; then
        rm -f "$dir/.crosgha.new"
        fail "the downloaded crosgha does not run on $platform; nothing was replaced"
    fi
    mv -f "$dir/.crosgha.new" "$dir/crosgha"
    printf 'Installed %s to %s/crosgha\n' "$installed" "$dir"

    case ":$PATH:" in
        *":$dir:"*) ;;
        *) printf 'Add %s to your PATH, e.g. in ~/.profile:\n  export PATH="%s:$PATH"\n' "$dir" "$dir" ;;
    esac
}

main "$@"
