# Output and exit status

| `--format` | Output |
|---|---|
| `text` (default; `plain` in zizmor) | One rustc-style diagnostic per finding: the source line with the node underlined, the rule's help, and the equivalent rule in other tools, such as `same as zizmor: template-injection; actionlint: expression`. A summary counts findings by severity and says when the scan is incomplete. |
| `json` | Versioned report: `version`, `summary`, `findings`, `errors` for workflows that failed to parse or to be written and for GitHub API failures that skipped online checks (`path` is `null`), and under `--fix` a `fixes` entry per proposed fix with its status (`applied`, `rejected`, or `not-written`) and reason. |
| `sarif` | SARIF 2.1.0 with the rule metadata GitHub code scanning requires; it validates against the official schema. Columns count UTF-16 code units (`columnKind`), and a repository-wide finding points at line 1 of the first scanned workflow of its `.github` directory. The JSON `errors` become `toolExecutionNotifications` of the invocation, whose `executionSuccessful` is then `false`. |
| `github` | Workflow-command annotations, without columns on a multi-line span; a repository-wide finding points at line 1 of the first scanned workflow of its `.github` directory. The default when `GITHUB_ACTIONS=true`. |
| `agent` | One brief line per finding for AI agents: `path:line:col: severity rule: location: message (zizmor: id; actionlint: id) help: <rule summary>`, with the equivalents only when the rule has any, after oxlint's format of the same name. |

`--sarif-file FILE` also writes SARIF during any format, so one run serves the log and the Security tab. After `--fix`,
every format lists what remains on disk: the findings left in each written file, and the original findings of a file
whose fixes were not written. Columns count characters in every format but SARIF. Text is colored on a terminal;
`NO_COLOR` turns it off, a non-empty `FORCE_COLOR` turns it on unless `NO_COLOR` is set, and `--color always|never`
overrides all of these.

| Exit status | Meaning |
|---|---|
| 0 | No finding reaches `--fail-on`. |
| 1 | A finding does. |
| 2 | A path, workflow, config, write, or API call failed. Findings in the workflows that did parse are still reported, and JSON and SARIF list the failed files. A workflow that fails to parse, or a failed API call, means no file is changed; a failed write leaves the other files fixed. A failed API call skips only the online checks that needed it; stderr names it. |
