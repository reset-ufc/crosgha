#!/usr/bin/env bash
set -euo pipefail

fail() {
    printf '::error::%s\n' "$1" >&2
    exit 1
}

[[ "${RUNNER_OS:-}" == Linux ]] || fail "crosgha Action supports Linux runners only"

case "${CROSGHA_FORMAT:-sarif}" in
    sarif|json|text) ;;
    *) fail "format must be sarif, json, or text" ;;
esac

case "${CROSGHA_FAIL_ON:-any}" in
    any|none|note|warning|error) ;;
    *) fail "fail-on must be any, none, note, warning, or error" ;;
esac

case "${CROSGHA_FIX:-none}" in
    none|pull-request) ;;
    *) fail "fix must be none or pull-request" ;;
esac

case "${CROSGHA_AUTO_MERGE:-false}" in
    true|false) ;;
    *) fail "dangerously-auto-merge must be true or false" ;;
esac

case "${CROSGHA_UPLOAD_SARIF:-false}" in
    true|false) ;;
    *) fail "upload-sarif must be true or false" ;;
esac

if [[ "${CROSGHA_FIX:-none}" == pull-request && -z "${CROSGHA_TOKEN:-}" ]]; then
    fail "fix: pull-request requires token; see ROADMAP.md"
fi
if [[ "${CROSGHA_AUTO_MERGE:-false}" == true && "${CROSGHA_FIX:-none}" != pull-request ]]; then
    fail "dangerously-auto-merge requires fix: pull-request"
fi
if [[ "${CROSGHA_UPLOAD_SARIF:-false}" == true && "${CROSGHA_FORMAT:-sarif}" != sarif ]]; then
    fail "upload-sarif requires format: sarif"
fi

# ponytail: This entry point validates the Action contract only. Add release
# resolution, checksum verification, reporting, PR creation, and auto-merge
# behind these checks when those paths have end-to-end fixtures.
fail "crosgha Action scaffold is not ready: verified installation and SARIF reporting are not implemented"
