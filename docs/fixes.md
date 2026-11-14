# Fixes

`--fix` applies a fix only when all of these hold:

1. the patched YAML parses;
2. the targeted finding disappears and no new finding appears;
3. only the intended YAML node changed.

A fix that fails a check is skipped: its finding stays in the report with the reason ("Not fixed" in text, `fixes` in
JSON), and the other fixes are still applied. CRLF line breaks and a missing final newline are kept. A symlinked
workflow is never rewritten; its fixes are reported as not written.

Five rules have fixes:

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
- `embedded-script-lint` makes two rewrites, one for each of its findings, which ghlint names
  `SafeEnvironmentFileRedirect` and `EnvironmentFileOverwritten`. Turning one of them off in the config keeps the other
  finding and its fix.

  **Quotes and braces** each environment file path that a `run` script redirects to, the form ShellCheck's
  [SC2086](https://www.shellcheck.net/wiki/SC2086) and [SC2250](https://www.shellcheck.net/wiki/SC2250) ask for:

  | Before | After |
  |---|---|
  | `echo "x=1" >> $GITHUB_OUTPUT` | `echo "x=1" >> "${GITHUB_OUTPUT}"` |
  | `echo "x=1" >> "$GITHUB_OUTPUT"` | `echo "x=1" >> "${GITHUB_OUTPUT}"` |
  | `echo "X=1" >${GITHUB_ENV}` | `echo "X=1" >"${GITHUB_ENV}"` |
  | `./lint 2>> $GITHUB_STEP_SUMMARY` | `./lint 2>> "${GITHUB_STEP_SUMMARY}"` |

  The fix is a whitelist: a path is rewritten only when it is the whole target word of a `>` or `>>` redirect in
  shell code, also inside `$( )`, `if` blocks, `{ }` groups, and on the first line of a heredoc. Only the path
  changes and the rest of the line keeps its bytes. The shell then writes to the path as one file name, as it did
  whenever the path held no spaces or glob characters; the braces change nothing, because a quote follows the name.
  A quoted path is reported only as a redirect target, as ghlint does. A step stays unfixed when the rule reports any
  path that is not such a target:

  - a command argument or an operand (`cat $GITHUB_ENV`, `[ -f $GITHUB_PATH ]`), a `for` list, a `case` subject or
    pattern, a `[[ ]]` operand, another variable's `${...}`, or an input redirect (`< $GITHUB_ENV`). There quoting
    can change what the script does: a `for` list and a pattern glob only unquoted;
  - text: inside single quotes, escaped (`\$GITHUB_ENV`), in a comment, in a heredoc body, or in a double-quoted
    string that spans lines;
  - a path in backticks, or a script that uses `$'...'` quoting, leaves a quote, `$(`, or `${` open, or has a
    heredoc without its delimiter line;
  - a folded (`>`) or quoted YAML scalar, which YAML rewrites before the shell sees it, or a plain scalar in a
    flow mapping (`{run: ...}`), where the added braces would end the scalar.

  **Appends** where `>` truncates an environment file, after ghlint's
  [`EnvironmentFileOverwritten`](https://github.com/TWiStErRob/net.twisterrob.ghlint/blob/74115a437cfa161c2fd2d72fc2ebcd83ac514b80/modules/ghlint-rules/src/main/kotlin/net/twisterrob/ghlint/rules/EnvironmentFileOverwriteRule.kt):

  | Before | After |
  |---|---|
  | `echo "x=1" > "${GITHUB_OUTPUT}"` | `echo "x=1" >> "${GITHUB_OUTPUT}"` |
  | `} > $GITHUB_ENV` | `} >> $GITHUB_ENV` |
  | `printf 'v=%s\n' "$v" 1>"${GITHUB_STATE}"` | `printf 'v=%s\n' "$v" 1>>"${GITHUB_STATE}"` |

  Only the operator changes; the quoting fix above handles the path. The runner
  [gives each step new, empty environment files](https://github.com/actions/runner/blob/8cfa1049f86781d141056082d73802d56cd2b433/src/Runner.Worker/FileCommandManager.cs), so `>` and `>>` write the same file unless the step wrote to it before: an earlier line, an
  earlier pass of a loop, or a program it ran. `>` dropped those writes and `>>` keeps them, so outputs, variables,
  and `PATH` entries that the step wrote first now reach later steps. A key written twice keeps its last value. The
  same reader decides what a redirect is, and a step stays unfixed when the rule still reports an overwrite:

  - `GITHUB_STEP_SUMMARY`: GitHub
    [documents `>`](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands#overwriting-job-summaries)
    as the way to clear the step's summary, so the fix cannot tell a mistake from intent;
  - `&>`, which sh reads as `&` and then `>`, and `<>`, which opens the file without truncating it;
  - a script with `[[` or `$[`, where `>` compares strings or numbers and the reader does not track the brackets;
  - a `>` the reader does not take for a redirect: escaped, in quotes, a comment, a heredoc body, or `$(( ))`,
    and every case of the quoting fix's list that hides the script.

  The rule reports only bash and sh steps, so other shells never get either fix.

- `unsafe-if-condition` rewrites a condition that mixes `${{ }}` with other text into one expression in a single
  `${{ }}`, the only form GitHub
  [evaluates](https://docs.github.com/en/actions/reference/workflows-and-actions/expressions) once `${{` appears in
  an `if`:

  | Before | After |
  |---|---|
  | `${{ github.event_name == 'push' }} && ${{ github.ref == 'refs/heads/main' }}` | `${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}` |
  | `${{ steps.size.outputs.lines }} == '0'` | `${{ steps.size.outputs.lines == '0' }}` |
  | `${{ a == 'x' \|\| b == 'y' }} && ${{ !c }}` | `${{ (a == 'x' \|\| b == 'y') && !c }}` |
  | `"!${{ cancelled() }}"` | `"${{ !cancelled() }}"` |
  | `\|` or `>` block holding `${{ ... }}` | `\|-` or `>-` with the same lines |

  This changes when the job or step runs, and that is the point. The mixed condition was a non-empty string, so it
  was always true; now it is the expression its operators spell out. A segment goes in parentheses when the operator
  next to it binds tighter than its own (`Precedence` in actions/runner's
  [`Token.cs`](https://github.com/actions/runner/blob/8cfa1049f86781d141056082d73802d56cd2b433/src/Sdk/DTExpressions2/Expressions2/Tokens/Token.cs)),
  so `&&` and `||` keep the grouping each `${{ }}` gave them. A block scalar gets the strip indicator, since the line
  break YAML keeps at its end would be text after `}}`; its lines and header comment stay. A quoted scalar keeps its
  quote style. The output is one wrapped expression, the form the `readability` check for bare conditions asks for,
  so the fix never adds a finding there. It stays unfixed when:

  - the text outside `${{ }}` holds anything but operators, parentheses, literals (`'text'`, numbers, `true`,
    `false`, `null`, `NaN`, `Infinity`), and whitespace: a context such as `github.ref`, a function such as `always()`, or prose. That
    text was never evaluated, so nothing says it is a valid expression;
  - the segments and that text do not form one expression: two segments with no operator between them, an empty
    `${{ }}`, or a `${{` without its `}}`;
  - the scalar is quoted over several lines, its block header has an indentation indicator (`|2`), or it carries
    an anchor or a tag, or is an alias, whose rewrite would change every alias;
  - the rewrite would be a new finding: a bot check such as `${{ github.actor }} == 'dependabot[bot]'` is spoofable
    once it is evaluated, so the fix leaves that choice to the author.

  Constant conditions (`if: true`) and bot checks are never fixed.

Every other rule is detection-only, because choosing a SHA, timeout, token scope, or trigger filter needs project
context.
