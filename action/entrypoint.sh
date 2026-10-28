#!/usr/bin/env bash
# Validates the Action inputs, installs the release binary after checking its sha256, and runs
# crosgha once. Findings do not fail this step: action.yml uploads SARIF first, then applies
# the `exit-code` output.
set -euo pipefail

# On stdout for the same reason as the stop and resume markers below: on stderr, the error
# could reach the runner while commands are still stopped.
fail() {
    printf '::error::%s\n' "$1"
    exit 1
}

boolean() {
    case "$2" in
        true|false) ;;
        *) fail "$1 must be true or false" ;;
    esac
}

# The release ships one asset, crosgha-x86_64-unknown-linux-musl.
[[ "${RUNNER_OS:-}" == Linux && "${RUNNER_ARCH:-}" == X64 ]] \
    || fail "crosgha Action supports Linux X64 runners only, not ${RUNNER_OS:-?} ${RUNNER_ARCH:-?}"

case "${CROSGHA_PERSONA:-regular}" in
    regular|pedantic|auditor) ;;
    *) fail "persona must be regular, pedantic, or auditor" ;;
esac

case "${CROSGHA_MIN_SEVERITY:-}" in
    ''|unknown|informational|low|medium|high) ;;
    *) fail "min-severity must be unknown, informational, low, medium, or high" ;;
esac

case "${CROSGHA_MIN_CONFIDENCE:-}" in
    ''|unknown|low|medium|high) ;;
    *) fail "min-confidence must be unknown, low, medium, or high" ;;
esac

case "${CROSGHA_FAIL_ON:-any}" in
    any|none|note|warning|error) ;;
    *) fail "fail-on must be any, none, note, warning, or error" ;;
esac

case "${CROSGHA_FIX:-none}" in
    none|pull-request) ;;
    *) fail "fix must be none or pull-request" ;;
esac

boolean online-audits "${CROSGHA_ONLINE_AUDITS:-true}"
boolean advanced-security "${CROSGHA_ADVANCED_SECURITY:-true}"
boolean annotations "${CROSGHA_ANNOTATIONS:-false}"
boolean dangerously-auto-merge "${CROSGHA_AUTO_MERGE:-false}"
boolean fail-on-no-inputs "${CROSGHA_FAIL_ON_NO_INPUTS:-true}"

if [[ "${CROSGHA_ANNOTATIONS:-false}" == true && "${CROSGHA_ADVANCED_SECURITY:-true}" == true ]]; then
    fail "annotations requires advanced-security: false"
fi
if [[ "${CROSGHA_FIX:-none}" == pull-request && -z "${CROSGHA_FIX_TOKEN:-}" ]]; then
    fail "fix: pull-request requires fix-token: a GitHub App token or fine-grained PAT with Contents, Pull requests, and Workflows: write"
fi
if [[ "${CROSGHA_AUTO_MERGE:-false}" == true && "${CROSGHA_FIX:-none}" != pull-request ]]; then
    fail "dangerously-auto-merge requires fix: pull-request"
fi
# ponytail: Report mode only; pull-request and auto-merge modes land once their branch, PR,
# and merge operations have end-to-end fixtures.
if [[ "${CROSGHA_FIX:-none}" == pull-request ]]; then
    fail "fix: pull-request is not implemented yet"
fi

# The release matching this Action revision: release-please bumps Cargo.toml in the tagged
# commit, so the Action's own Cargo.toml names its release.
manifest="$GITHUB_ACTION_PATH/Cargo.toml"
version="${CROSGHA_VERSION:-}"
if [[ -z "$version" ]]; then
    version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$manifest" | head -n1)"
fi
version="${version#v}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]] || fail "version must be X.Y.Z, got '$version'"

repository="$(sed -n 's|^repository = "https://github.com/\(.*\)"$|\1|p' "$manifest" | head -n1)"
base="${CROSGHA_RELEASE_BASE:-https://github.com/$repository/releases/download}"
protocol='=https'
# Tests serve a local release over plain HTTP; nothing else may.
[[ "$base" =~ ^http://127\.0\.0\.1:[0-9]+$ ]] && protocol='=http'

asset=crosgha-x86_64-unknown-linux-musl.tar.xz
install_dir="$RUNNER_TEMP/crosgha-$version"
mkdir -p "$install_dir"
download() {
    curl --proto "$protocol" --tlsv1.2 -fsSL --retry 3 -o "$install_dir/$1" "$base/v$version/$1" \
        || fail "cannot download $1 for crosgha v$version"
}
download "$asset"
download sha256.sum
# Releases up to v0.9.0 write `*name` (binary mode). A missing line leaves sha256sum with no
# input, which fails like a mismatch.
(cd "$install_dir" && awk -v asset="$asset" '$2 == asset || $2 == "*" asset' sha256.sum \
    | sha256sum --check --status -) \
    || fail "checksum mismatch for $asset; refusing to run it"
tar -xJf "$install_dir/$asset" -C "$install_dir"
crosgha="$install_dir/crosgha-x86_64-unknown-linux-musl/crosgha"
[[ -x "$crosgha" ]] || fail "release archive has no crosgha binary"

args=(--persona "${CROSGHA_PERSONA:-regular}" --fail-on "${CROSGHA_FAIL_ON:-any}")
[[ -n "${CROSGHA_MIN_SEVERITY:-}" ]] && args+=(--min-severity "$CROSGHA_MIN_SEVERITY")
[[ -n "${CROSGHA_MIN_CONFIDENCE:-}" ]] && args+=(--min-confidence "$CROSGHA_MIN_CONFIDENCE")
[[ -n "${CROSGHA_CONFIG_FILE:-}" ]] && args+=(--config "$CROSGHA_CONFIG_FILE")
if [[ "${CROSGHA_ONLINE_AUDITS:-true}" == true && -n "${CROSGHA_TOKEN:-}" ]]; then
    export GH_TOKEN="$CROSGHA_TOKEN"
else
    unset GH_TOKEN GITHUB_TOKEN
    args+=(--offline)
fi
# Explicit either way: crosgha defaults to annotations when GITHUB_ACTIONS=true.
if [[ "${CROSGHA_ANNOTATIONS:-false}" == true ]]; then
    args+=(--format github)
else
    args+=(--format plain)
fi
sarif="$RUNNER_TEMP/crosgha.sarif"
args+=(--sarif-file "$sarif")

# Block scalars put one path per line, so split on newlines too.
read -r -d '' -a inputs <<< "${CROSGHA_INPUTS:-.}" || true
report="$RUNNER_TEMP/crosgha.txt"
set +e
"$crosgha" "${args[@]}" -- "${inputs[@]}" > "$report" 2> "$RUNNER_TEMP/crosgha.err"
code=$?
set -e

# File names and YAML keys from the scanned repo reach the report and stderr, so a line there could
# read as a workflow command. Annotation output is crosgha's own escaped commands and stays live.
# Everything goes to stdout because the runner reads stdout and stderr independently, so text on
# stderr is not guaranteed to land between the stop and resume markers.
token="$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')"
[[ "${CROSGHA_ANNOTATIONS:-false}" == true ]] && cat "$report"
echo "::stop-commands::$token"
[[ "${CROSGHA_ANNOTATIONS:-false}" == true ]] || cat "$report"
cat "$RUNNER_TEMP/crosgha.err"
echo "::$token::"

if [[ "$code" == 2 ]] && grep -q "no .yml or .yaml workflow files found" "$RUNNER_TEMP/crosgha.err" \
    && [[ "${CROSGHA_FAIL_ON_NO_INPUTS:-true}" == false ]]; then
    printf '::warning::crosgha found no workflow files in %s\n' "${inputs[*]}"
    echo "findings=0" >> "$GITHUB_OUTPUT"
    echo "exit-code=0" >> "$GITHUB_OUTPUT"
    exit 0
fi
# 0 is clean and 1 is findings. Anything else (2, a panic's 101, a binary that cannot execute's
# 126) leaves no complete report or SARIF to publish.
[[ "$code" == 0 || "$code" == 1 ]] || fail "crosgha could not complete the scan (exit status $code)"

# The SARIF renderer writes one `ruleId` key per result.
findings="$(grep -c '"ruleId":' "$sarif" || true)"
{
    echo "findings=$findings"
    echo "exit-code=$code"
    [[ "${CROSGHA_ADVANCED_SECURITY:-true}" == true ]] && echo "output-file=$sarif"
} >> "$GITHUB_OUTPUT"
# A fence longer than every backtick run in the report keeps the whole report inside the block.
fence="$(awk '{ while (match($0, /`+/)) { if (RLENGTH > max) max = RLENGTH; $0 = substr($0, RSTART + RLENGTH) } }
    END { for (n = (max < 3 ? 3 : max + 1); n > 0; n--) printf "`" }' "$report")"
{
    echo "### crosgha v$version: $findings finding(s)"
    echo
    echo "${fence}text"
    cat "$report"
    echo "$fence"
} >> "$GITHUB_STEP_SUMMARY"
