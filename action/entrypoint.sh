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
boolean follow-actions "${CROSGHA_FOLLOW_ACTIONS:-false}"

if [[ "${CROSGHA_ANNOTATIONS:-false}" == true && "${CROSGHA_ADVANCED_SECURITY:-true}" == true ]]; then
    fail "annotations requires advanced-security: false"
fi
if [[ "${CROSGHA_FIX:-none}" == pull-request && -z "${CROSGHA_FIX_TOKEN:-}" ]]; then
    fail "fix: pull-request requires fix-token: a GitHub App token or fine-grained PAT with Contents, Pull requests, and Workflows: write"
fi
if [[ "${CROSGHA_FOLLOW_ACTIONS:-false}" == true \
    && ( "${CROSGHA_ONLINE_AUDITS:-true}" != true || -z "${CROSGHA_TOKEN:-}" ) ]]; then
    fail "follow-actions requires online-audits: true and a token"
fi
if [[ "${CROSGHA_AUTO_MERGE:-false}" == true && "${CROSGHA_FIX:-none}" != pull-request ]]; then
    fail "dangerously-auto-merge requires fix: pull-request"
fi
# ponytail: auto-merge lands once its merge operations have end-to-end fixtures (#73).
if [[ "${CROSGHA_AUTO_MERGE:-false}" == true ]]; then
    fail "dangerously-auto-merge is not implemented yet"
fi
if [[ "${CROSGHA_FIX:-none}" == pull-request ]]; then
    [[ "${GITHUB_REF:-}" == refs/heads/* ]] \
        || fail "fix: pull-request proposes fixes against a branch, and ${GITHUB_REF:-this run} is not one; run it on push, schedule, or workflow_dispatch"
    command -v jq > /dev/null || fail "fix: pull-request needs jq on the runner"
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

# Never colored: the report goes to a file that the job summary, Markdown, may copy, whatever
# FORCE_COLOR the caller's job sets.
# `common` is shared with the fix run of pull-request mode.
common=(--color never --persona "${CROSGHA_PERSONA:-regular}")
[[ -n "${CROSGHA_MIN_SEVERITY:-}" ]] && common+=(--min-severity "$CROSGHA_MIN_SEVERITY")
[[ -n "${CROSGHA_MIN_CONFIDENCE:-}" ]] && common+=(--min-confidence "$CROSGHA_MIN_CONFIDENCE")
[[ -n "${CROSGHA_CONFIG_FILE:-}" ]] && common+=(--config "$CROSGHA_CONFIG_FILE")
args=(--fail-on "${CROSGHA_FAIL_ON:-any}")
if [[ "${CROSGHA_ONLINE_AUDITS:-true}" == true && -n "${CROSGHA_TOKEN:-}" ]]; then
    export GH_TOKEN="$CROSGHA_TOKEN"
    [[ "${CROSGHA_FOLLOW_ACTIONS:-false}" == true ]] && args+=(--follow-actions)
else
    unset GH_TOKEN GITHUB_TOKEN
    common+=(--offline)
fi
args=("${common[@]}" "${args[@]}")
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
# crosgha stops at the first input without workflows, so tolerating them means finding them first:
# each input is probed alone, and only the ones with workflows are scanned.
if [[ "${CROSGHA_FAIL_ON_NO_INPUTS:-true}" == false ]]; then
    kept=()
    empty=()
    for input in "${inputs[@]}"; do
        if ! "$crosgha" --offline --no-config --fail-on none --format json -- "$input" \
                > /dev/null 2> "$RUNNER_TEMP/crosgha.probe" \
            && grep -q "no .yml or .yaml workflow files found" "$RUNNER_TEMP/crosgha.probe"; then
            empty+=("$input")
        else
            kept+=("$input")
        fi
    done
    if (( ${#empty[@]} )); then
        printf '::warning::crosgha found no workflow files in %s\n' "${empty[*]}"
    fi
    if (( ${#kept[@]} == 0 )); then
        echo "findings=0" >> "$GITHUB_OUTPUT"
        echo "exit-code=0" >> "$GITHUB_OUTPUT"
        exit 0
    fi
    inputs=("${kept[@]}")
fi
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
{
    echo "### crosgha v$version: $findings finding(s)"
    echo
    # Annotation output is workflow commands, which mean nothing in Markdown.
    if [[ "${CROSGHA_ANNOTATIONS:-false}" == true ]]; then
        echo "Findings are annotations in this run's log."
    else
        # A fence longer than every backtick run in the report keeps the whole report inside the block.
        fence="$(awk '{ while (match($0, /`+/)) { if (RLENGTH > max) max = RLENGTH; $0 = substr($0, RSTART + RLENGTH) } }
            END { for (n = (max < 3 ? 3 : max + 1); n > 0; n--) printf "`" }' "$report")"
        echo "${fence}text"
        cat "$report"
        echo "$fence"
    fi
} >> "$GITHUB_STEP_SUMMARY"

[[ "${CROSGHA_FIX:-none}" == pull-request ]] || exit 0

# Pull-request mode. The fixes are committed through a separate index, so the caller's HEAD, index,
# and other changes stay as they were, and only the files crosgha wrote enter the commit.
quoted() {
    local token
    token="$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')"
    echo "::stop-commands::$token"
    cat "$@"
    echo "::$token::"
}
# The checkout as the earlier steps left it, to tell their edits from crosgha's.
snapshot="$RUNNER_TEMP/crosgha-snapshot-index"
GIT_INDEX_FILE="$snapshot" git read-tree HEAD
GIT_INDEX_FILE="$snapshot" git add --all
before="$(GIT_INDEX_FILE="$snapshot" git write-tree)"
fixes="$RUNNER_TEMP/crosgha-fixes.json"
set +e
"$crosgha" "${common[@]}" --fix --fail-on none --format json -- "${inputs[@]}" \
    > "$fixes" 2> "$RUNNER_TEMP/crosgha-fix.err"
code=$?
set -e
if [[ "$code" != 0 ]]; then
    quoted "$RUNNER_TEMP/crosgha-fix.err"
    fail "crosgha --fix could not complete (exit status $code)"
fi
jq -j '[.fixes[] | select(.status == "applied") | .path] | unique | .[] + "\u0000"' "$fixes" \
    > "$RUNNER_TEMP/crosgha-fixed" || fail "cannot read the fixes crosgha reported"
mapfile -d '' fixed < "$RUNNER_TEMP/crosgha-fixed"
if (( ${#fixed[@]} == 0 )); then
    printf '\nNo fix to propose.\n' >> "$GITHUB_STEP_SUMMARY"
    exit 0
fi
git diff --name-only HEAD "$before" -- "${fixed[@]}" > "$RUNNER_TEMP/crosgha-edited"
if [[ -s "$RUNNER_TEMP/crosgha-edited" ]]; then
    quoted "$RUNNER_TEMP/crosgha-edited"
    fail "fix: pull-request commits only crosgha's fixes, and the files above already differed from HEAD before them"
fi

branch="crosgha/fix-${GITHUB_RUN_ID:?}"
base_branch="${GITHUB_REF#refs/heads/}"
server="${GITHUB_SERVER_URL:-https://github.com}"
title="Fix GitHub Actions smells found by crosgha"
summary="$(jq -r '[.fixes[] | select(.status == "applied") | "- `\(.path):\(.line)` \(.rule): \(.location)"] | join("\n")' "$fixes")"
export GIT_AUTHOR_NAME='github-actions[bot]' GIT_COMMITTER_NAME='github-actions[bot]'
export GIT_AUTHOR_EMAIL='41898282+github-actions[bot]@users.noreply.github.com'
export GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"
index="$RUNNER_TEMP/crosgha-index"
GIT_INDEX_FILE="$index" git read-tree HEAD
GIT_INDEX_FILE="$index" git add -- "${fixed[@]}"
tree="$(GIT_INDEX_FILE="$index" git write-tree)"
commit="$(printf '%s\n\n%s\n' "$title" "$summary" | git commit-tree "$tree" -p HEAD)"

# The token travels as checkout's header does, through the environment rather than the command
# line. The empty entry drops a header that a checkout with persist-credentials left behind.
header="AUTHORIZATION: basic $(printf 'x-access-token:%s' "$CROSGHA_FIX_TOKEN" | base64 | tr -d '\n')"
if ! GIT_CONFIG_COUNT=2 \
    GIT_CONFIG_KEY_0="http.$server/.extraheader" GIT_CONFIG_VALUE_0='' \
    GIT_CONFIG_KEY_1="http.$server/.extraheader" GIT_CONFIG_VALUE_1="$header" \
    git push --quiet "$server/${GITHUB_REPOSITORY:?}.git" "$commit:refs/heads/$branch" \
    2> "$RUNNER_TEMP/crosgha-push.err"; then
    quoted "$RUNNER_TEMP/crosgha-push.err"
    fail "cannot push $branch; fix-token needs Contents and Workflows: write, and the branch must not exist yet"
fi

api="${GITHUB_API_URL:-https://api.github.com}"
api_protocol='=https'
# Tests serve the API over plain HTTP; nothing else may.
[[ "$api" =~ ^http://127\.0\.0\.1:[0-9]+$ ]] && api_protocol='=http'
request="$RUNNER_TEMP/crosgha-pull-request.json"
jq --arg title "$title" --arg head "$branch" --arg base "$base_branch" --arg version "$version" \
    '{title: $title, head: $head, base: $base, body: ("crosgha v\($version) fixed these findings, each checked to re-parse, to remove its finding without adding one, and to change only its own YAML node:\n\n" + ([.fixes[] | select(.status == "applied") | "- `\(.path):\(.line)` \(.rule): \(.location)"] | join("\n")))}' \
    "$fixes" > "$request"
# A header file keeps the token off the command line.
authorization="$RUNNER_TEMP/crosgha-authorization"
(umask 077 && printf 'Authorization: Bearer %s\n' "$CROSGHA_FIX_TOKEN" > "$authorization")
response="$RUNNER_TEMP/crosgha-pull-response.json"
status="$(curl --proto "$api_protocol" --tlsv1.2 -sS -o "$response" -w '%{http_code}' -X POST \
    -H @"$authorization" -H 'Accept: application/vnd.github+json' \
    -H 'X-GitHub-Api-Version: 2022-11-28' --data-binary @"$request" \
    "$api/repos/$GITHUB_REPOSITORY/pulls")" || status=000
rm -f "$authorization"
if [[ "$status" != 201 ]]; then
    message="$(jq -r '.message // empty' "$response" 2> /dev/null || true)"
    fail "cannot open the pull request for $branch (HTTP $status): ${message:-no message}"
fi
url="$(jq -r '.html_url' "$response")"
echo "pull-request-url=$url" >> "$GITHUB_OUTPUT"
printf '\nProposed %s fix(es) in %s\n' "$(grep -c '^- ' <<< "$summary")" "$url" >> "$GITHUB_STEP_SUMMARY"
