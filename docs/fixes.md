# Fixes

`--fix` applies a fix only when all of these hold:

1. the patched YAML parses;
2. the targeted finding disappears and no new finding appears;
3. only the intended YAML node changed.

A fix that fails a check is skipped: its finding stays in the report with the reason ("Not fixed" in text, `fixes` in
JSON), and the other fixes are still applied. CRLF line breaks and a missing final newline are kept. A symlinked
workflow is never rewritten; its fixes are reported as not written.

Two rules have fixes:

- `persist-credentials` adds `persist-credentials: false`. It leaves the finding unfixed, with the reason in its
  message, when `true` is explicit, when a later step of the job pushes with the persisted token (`git push` or
  `stefanzweifel/git-auto-commit-action`), or for actions/checkout before v2.
- `removed-feature` replaces the AdoptOpenJDK distributions that setup-java v6 removed with the successor its README
  names.

Every other rule is detection-only, because choosing a SHA, timeout, token scope, or trigger filter needs project
context.
