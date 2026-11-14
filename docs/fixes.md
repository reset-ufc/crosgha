# Fixes

`--fix` applies a fix only when all of these hold:

1. the patched YAML parses;
2. the targeted finding disappears and no new finding appears;
3. only the intended YAML node changed.

A fix that fails a check is skipped: its finding stays in the report with the reason ("Not fixed" in text, `fixes` in
JSON), and the other fixes are still applied. CRLF line breaks and a missing final newline are kept. A symlinked
workflow is never rewritten; its fixes are reported as not written.

Four rules have fixes:

- `persist-credentials` adds `persist-credentials: false`. It leaves the finding unfixed, with the reason in its
  message, when `true` is explicit, when a later step of the job pushes with the persisted token (`git push` or
  `stefanzweifel/git-auto-commit-action`), or for actions/checkout before v2.
- `removed-feature` replaces the AdoptOpenJDK distributions that setup-java v6 removed with the successor its README
  names.
- `deprecated-command` rewrites each workflow command of a `run` script into the environment file write that
  [replaces it](https://github.blog/changelog/2022-10-11-github-actions-deprecating-save-state-and-set-output-commands/):

  | Before | After |
  |---|---|
  | `echo "::set-output name=x::1"` | `echo "x=1" >> "${GITHUB_OUTPUT}"` |
  | `echo "::save-state name=x::1"` | `echo "x=1" >> "${GITHUB_STATE}"` |
  | `echo "::set-env name=X::1"` | `echo "X=1" >> "${GITHUB_ENV}"` |
  | `echo "::add-path::/opt/bin"` | `printf '%s\n' "/opt/bin" >> "${GITHUB_PATH}"` |

  The write keeps the line's quoting (double, single, or none), so the shell expands the value as it did before,
  and the line keeps its indentation inside `if` or loop blocks. A named write keeps `echo`, since its argument
  starts with `name=` and echo never reads it as an option. A path uses `printf`, because echo would take a path
  such as `-n` (written or expanded) as an option and write nothing. A step is rewritten only when every
  command in it has a rewrite. It stays unfixed when:

  - the step does not run bash or sh: `shell: pwsh`, `python`, or `cmd`, or a Windows runner's default pwsh;
  - the script is a folded (`>`) or quoted YAML scalar, which YAML rewrites before the shell sees it;
  - the line is not exactly one `echo` with one argument: `printf`, `echo -n`, a pipe, `;`, a redirect, or a trailing
    comment;
  - the name is not literal (`[A-Za-z0-9_-]`), the value holds its own quote character, a double-quoted value ends
    with `\`, or an unquoted value holds anything but plain characters and `$NAME`/`${NAME}` (globs, braces,
    substitutions);
  - an `add-path` path holds `\` (sh's echo expanded escapes that `printf '%s'` keeps), or is unquoted with a space
    or `$` (it could split into several words, each a line of its own);
  - the script holds `%25`, `%0D`, or `%0A`: the runner decoded these in a command's value, an environment file
    stores them as written, so a script that escapes multi-line values that way would write something else;
  - the command sits in a heredoc, a quoted string that spans lines, or a line continued with `\`, or the script
    uses `$'...'` quoting;
  - the rewrite would be a new finding, such as `github-env-injection` for `set-env` or `add-path` in a privileged
    workflow.

  A value that expands to several lines at run time changes behavior: the old command used only the first line,
  while the runner rejects the extra lines of an environment file. Use the
  [multiline syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#multiline-strings)
  for such values.
- `embedded-script-lint` quotes and braces each environment file path that a `run` script expands unquoted, the
  form ShellCheck's [SC2086](https://www.shellcheck.net/wiki/SC2086) and
  [SC2250](https://www.shellcheck.net/wiki/SC2250) ask for:

  | Before | After |
  |---|---|
  | `echo "x=1" >> $GITHUB_OUTPUT` | `echo "x=1" >> "${GITHUB_OUTPUT}"` |
  | `echo "X=1" >${GITHUB_ENV}` | `echo "X=1" >"${GITHUB_ENV}"` |
  | `cat $GITHUB_ENV` | `cat "${GITHUB_ENV}"` |

  Only the path changes: `>` stays `>`, the rest of the line keeps its bytes, and every unquoted path of the
  step is quoted, also inside `$( )`, `if` blocks, `{ }` groups, and on the first line of a heredoc. The shell
  then reads the path as one word, as it did whenever the path held no spaces or glob characters. A step stays
  unfixed when a path the rule reports is not one the shell expands unquoted, or when the reader cannot tell:

  - the path is text: inside single quotes, escaped (`\$GITHUB_ENV`), in a comment, in a heredoc body, or in a
    double-quoted string that spans lines;
  - the path sits in backticks, or the script uses `$'...'` quoting, leaves a quote or `$(` open, or has a heredoc
    without its delimiter line;
  - the script is a folded (`>`) or quoted YAML scalar, which YAML rewrites before the shell sees it, or a plain
    scalar in a flow mapping (`{run: ...}`), where the added braces would end the scalar.

  The rule reports only bash and sh steps, so other shells never get the fix.

Every other rule is detection-only, because choosing a SHA, timeout, token scope, or trigger filter needs project
context.
