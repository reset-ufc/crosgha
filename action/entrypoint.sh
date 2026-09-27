#!/usr/bin/env bash
set -euo pipefail

fail() {
    printf '::error::%s\n' "$1" >&2
    exit 1
}

boolean() {
    case "$2" in
        true|false) ;;
        *) fail "$1 must be true or false" ;;
    esac
}

[[ "${RUNNER_OS:-}" == Linux ]] || fail "crosgha Action supports Linux runners only"

case "${CROSGHA_PERSONA:-regular}" in
    regular|pedantic|auditor) ;;
    *) fail "persona must be regular, pedantic, or auditor" ;;
esac

case "${CROSGHA_MIN_SEVERITY:-}" in
    ''|unknown|informational|low|medium|high) ;;
    *) fail "min-severity must be unknown, informational, low, medium, or high" ;;
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

if [[ "${CROSGHA_ANNOTATIONS:-false}" == true && "${CROSGHA_ADVANCED_SECURITY:-true}" == true ]]; then
    fail "annotations requires advanced-security: false"
fi
if [[ "${CROSGHA_FIX:-none}" == pull-request && -z "${CROSGHA_FIX_TOKEN:-}" ]]; then
    fail "fix: pull-request requires fix-token; see ROADMAP.md"
fi
if [[ "${CROSGHA_AUTO_MERGE:-false}" == true && "${CROSGHA_FIX:-none}" != pull-request ]]; then
    fail "dangerously-auto-merge requires fix: pull-request"
fi

# ponytail: This entry point validates the Action contract only. Add release
# resolution, checksum verification, reporting, PR creation, and auto-merge
# behind these checks when those paths have end-to-end fixtures.
fail "crosgha Action scaffold is not ready: verified installation and SARIF reporting are not implemented"
