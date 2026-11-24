# Usage

## Scanning

Each `PATH` may be a workflow file, an action file, a directory of workflows, a repository root, or its `.github`; the
last two are scanned through `.github/workflows` as zizmor does with `.`. Subdirectories are never scanned: GitHub does
not load workflows from them. Without a `PATH`, crosgha scans `.github/workflows`. A `dependabot.yml` path gets only the
Dependabot cooldown check. Symlinked files, and files reached through a symlinked directory below the `PATH`, are
scanned but never rewritten; a symlink to another scanned workflow or action is reported and fixed once, through its
target.

### Actions

A file named `action.yml` or `action.yaml` is an [action metadata
file](https://docs.github.com/en/actions/reference/workflows-and-actions/metadata-syntax), unless it sits in
`.github/workflows`, where it is a workflow (zizmor tells them apart the same way).

- A file named as a `PATH` is always scanned, and so is an action file directly inside a directory `PATH`. In a directory,
  `action.yaml` is skipped when `action.yml` is next to it, because the runner then never reads it.
- Scanning a repository root, its `.github`, or its `.github/workflows` (also the default `PATH`) adds the actions the
  repository runs or publishes: the root `action.yml` (or `action.yaml`), and each local action a scanned workflow
  calls with `uses: ./path`, followed through the composite actions it calls in turn. Paths resolve from the repository
  root, the caller's checkout. Each action is scanned once, a cycle included; a `uses: ./path` without a metadata file
  (a missing directory, or a Docker action with only a `Dockerfile`) is skipped, and so, with a warning, is one that
  resolves outside the repository through `..` or a symbolic link. crosgha does not walk the other directories as
  zizmor does, so an action that no workflow calls, such as one published from a subdirectory, is scanned only when
  given as a `PATH`. A repository with a root action and no workflows is not an empty input.
- A composite action's `runs.steps` get every rule that reads steps, `uses` lines, or text anywhere in the file;
  JavaScript and Docker actions have no steps, so only those file-wide checks and the metadata checks apply. Rules that
  read triggers, jobs, `permissions`, timeouts, concurrency, or `defaults` never run on an action. The **Actions**
  column of the [rule catalog](rules.md) lists what each rule checks, and `--list-rules` prints it last.
- In a composite action, `script-injection` also reports `${{ inputs.<name> }}` expanded into a `run` script or
  `actions/github-script`'s `script`, since the caller sets every input, possibly from event data. Pass the input
  through `env` instead.
- An action without a `runs` mapping or a string `runs.using` cannot be parsed, like a workflow without `on` or `jobs`:
  it is reported on stderr, the other files are still checked, the run exits 2, and `--fix` writes nothing. Other
  schema problems, such as a composite `run` step without `shell`, are `schema-validity` findings.
- Action files use the same ignore comments, `crosgha.yml` or `zizmor.yml` of the input (an action found through a
  repository uses that repository's), and verified fixes. `persist-credentials` is never fixed in an action, because
  the steps that run after it are in the caller's workflow.
- Remote inputs find their repository's actions the same way (below). Actions called from other repositories are
  checked with `--follow-actions` ([below](#actions-from-other-repositories)).

### Remote repositories

`crosgha owner/repo@ref` scans a GitHub repository without a clone. Omit `@ref` to scan the default branch; a local
path with the same spelling takes precedence, and `https://github.com/owner/repo@ref` is also accepted. Remote inputs
require `--gh-token`, `GH_TOKEN`, or `GITHUB_TOKEN`; private repositories outside the caller's repository need a token
that can read them. `--offline` and `--fix` reject them before any request.

- The ref is resolved to a commit first, and every file is read at that SHA, which the report names
  (`owner/repo@<sha>:.github/workflows/ci.yml`).
- The workflows directly in `.github/workflows`, the root action and the local actions they call ([Actions](#actions)),
  the repository rules (`missing-release-cooldown`, `missing-security-tooling`), and `repo-org-settings` report what a
  local clone of that commit reports, and repository-wide findings are placed on the same first workflow.
- Config uses the repository's own `crosgha.yml` or `zizmor.yml`, at its root or in `.github`, in the order of
  [Config file](configuration.md#config-file). `--config` (or its variables) and `--no-config` work as for local
  inputs. In a run with several inputs, each local and remote input uses its own repository's config.
- `repo-org-settings` checks the remote repository itself. `--repository` (and `GITHUB_REPOSITORY`, which names the
  calling repository in Actions) applies to local inputs only.
- Symbolic links and submodules are not followed. The API lists a link without its target and a submodule's files live
  in another repository, so a workflow, action, or config reached only through one is skipped. Locally, links are
  followed.
- A workflow or action the tree lists that the API refuses (403, 404, a server error) is reported for that file, the
  rest is still checked, and the run exits 2. A missing commit, or an unreadable tree, config, or Dependabot file, stops
  the run with exit 2. After a rate limit or a rejected token no further request is sent, so every later read fails the
  same way.

Request cost per remote repository, before online rules (`--no-online-audits` skips those, not these reads):

| Requests | For |
|---|---|
| 1 | the commit the ref names |
| 1 | the recursive tree, which answers every "does this file exist" check. When GitHub truncates it (over 100,000 entries or 7 MB), 1 more per directory a lookup needs: the root, `.github`, `.github/workflows`, and the directories on the way to each called action |
| 1 per file | each `.yml`/`.yaml` workflow |
| 0 or 1 | the config file found (none with `--config` or `--no-config`) |
| 1 per file | each action metadata file found: the root `action.yml` and each local action reached |
| 0 to 2 | `.github/dependabot.yml` and `.yaml`, when they exist and `missing-release-cooldown` runs (`pedantic`) |
| 0 or 1 | the Actions settings for `repo-org-settings` (`pedantic`, online rules enabled) |

A repository with 5 workflows, a root action, two local actions, a `zizmor.yml`, and a Dependabot config costs
1 + 1 + 5 + 1 + 3 + 1 = 12 requests at `--persona pedantic`, before `repo-org-settings` and the online rules.

### Actions from other repositories

`--follow-actions` also checks the actions that the scanned files' steps call from other repositories
(`uses: owner/repo[/path]@ref`). It is off by default because it spends GitHub API requests, and it needs a token
(`--gh-token`, `GH_TOKEN`, or `GITHUB_TOKEN`); `--offline`, `--no-online-audits`, and `--fix` refuse it. The Action
turns it on with `follow-actions: true`.

- Each ref that is not a full 40-character SHA is resolved to a commit first, and the action's `action.yml`, or
  `action.yaml` when there is none, is read at that commit, as the runner reads them. A followed file is reported as
  `owner/repo@<sha>:path/action.yml`, so the report names the commit it analyzed; a tag or branch that moves later is a
  different commit.
- Followed files run the same rules as local action files ([Actions](#actions)): a composite action's steps get the
  step rules, and JavaScript and Docker actions only the metadata checks (`retired-runtime`, an unpinned `docker://`
  image, `schema-validity`). Online rules do not run on followed files, so the request cost stays the one below. The
  scanned input's config applies, as do `--persona` and `--min-severity`.
- `uses:` lines inside a composite action's steps are followed transitively. `$/path` in a followed action is that
  action's repository at the same commit, as the runner resolves it. Not followed: `./path` inside another repository's
  action, which is the caller's checkout; `docker://` images; references built with `${{ }}`; and a job's `uses:`, which
  calls a reusable workflow, not an action. Owner and repository names ignore case, and a trailing `/` or empty segments
  in the path name the same directory, so those spellings are read once.
- Each action is identified by owner, repository, path, and commit, so a cycle stops and an action that several chains
  reach is read and checked once. The same action at two commits is two actions. Followed composite actions are nested
  at most 9 levels below the scanned step, the runner's limit (`CompositeActionsMaxDepth` in actions/runner); calls
  deeper than that are skipped with a warning. The limit holds per scanned line: an action read because a shorter line
  reaches it is not reported through a line whose chain to it would be deeper.
- Every finding names the `uses:` lines that lead to it, starting at a line of a scanned file. Text, annotations, SARIF,
  and agent lines show the shortest chain from the first such line and how many other scanned lines also lead there;
  JSON lists one chain per scanned line under `via` ([Output](output.md)). Annotations, SARIF, and agent lines are placed
  on the scanned `uses:` line, since code scanning shows only files of the scanned repository, and the message names the
  action's file and line.
- Ignore comments inside another repository's action are not honored: its author wrote them, not the scanned repository.
  An ignore comment on a scanned `uses:` line (`# crosgha: ignore[script-injection]`) applies to that line and to
  everything it pulls in, and so does a config `ignore` entry for that file and line (`ci.yml:12`). A finding stays
  reported while any scanned line that reaches it does not ignore it. `disable`, severity remaps, and `--no-ignores`
  work as everywhere.
- `--fix` cannot be combined with `--follow-actions`; a followed file belongs to another repository.
- A missing repository, ref, or metadata file (a Dockerfile-only action, a wrong path, a SHA outside the repository) is
  a warning on stderr and leaves the exit status alone. A metadata file that does not parse is a file error. Another
  failed request (5xx, network, 403) skips that action with a warning, and a rate limit or rejected token stops every
  later request and is reported once with the number of actions left unread. In those cases the findings already found
  are still reported, the scan is incomplete, and the run exits 2.

Request cost, cached for the whole run, failures included:

| Requests | For |
|---|---|
| 1 per distinct repository and ref | resolving a ref that is not a full SHA to a commit |
| 1 per distinct action and commit | `action.yml` |
| 0 or 1 per distinct action and commit | `action.yaml`, only when `action.yml` is missing |

Against the test API, `tests/fixtures/follow/caller/.github/workflows/ci.yml` costs 20 requests: 9 ref lookups (6
resolve, 3 name nothing) and 11 file reads (9 for the 8 actions read, one of which has only `action.yaml`, and 2 for a
SHA the repository does not have). `GITHUB_TOKEN` gets "1,000 requests per hour per
repository" ([rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)),
shared with the online rules and the rest of the job.

## Pre-push hook

```sh
crosgha hook install -- --offline --persona pedantic
crosgha hook uninstall
```

`install` writes a git `pre-push` hook in the directory git resolves for hooks, so `core.hooksPath` is respected. It
refuses a hooks directory outside the repository, such as a global `core.hooksPath`, where the hook would run on every
push from every repository.

- Everything after `--` is validated and passed to every scan. Re-running `install` replaces them. Subcommands,
  `--fix`, `--list-rules`, `--version`, and `--help` are refused: the hook only scans.
- It runs `crosgha` from `PATH`, at the repository root, on the working tree (uncommitted edits count).
- When findings remain, it blocks the push and tells you to run `crosgha --fix`. It never edits files itself.
- Both commands refuse to touch a `pre-push` hook that crosgha did not write. A hook counts as crosgha's only if it
  matches the generated script exactly, so an edited copy is yours.

## Migrating from zizmor, actionlint, poutine, ghalint, or octoscan

`crosgha migrate --from zizmor|actionlint|poutine|ghalint|octoscan [PATH]` moves a repository over:

- `zizmorcore/zizmor-action`, `raven-actions/actionlint`, `reviewdog/action-actionlint`, and `rhysd/actionlint` steps
  become crosgha Action steps. zizmor steps keep their `with:` block (the crosgha Action accepts zizmor-action's
  inputs); actionlint steps lose theirs. A step written as a flow mapping (`- { uses: ... }`) stays, with a note.
- For poutine, `boostsecurityio/poutine-action` steps become crosgha Action steps with `fail-on: none`, since poutine
  fails only with `--fail-on-violation`. A `github/codeql-action/upload-sarif` step of the same job that uploads the
  poutine step's SARIF file is removed, because the crosgha Action uploads its own; without one, the step gets
  `advanced-security: 'false'`. A poutine step that writes another `format`, or whose report file a later step of the job reads, stays, with a note. In shell steps,
  `brew install poutine` installs it and only `poutine analyze_local` translates, since the other commands read
  repositories through an SCM API. `--token` becomes `token`; `--format pretty`, `--verbose`, `--threads`,
  `--disable-version-check`, `--ignore-forks`, `--scm github`, and `--scm-base-url` are dropped; any other flag keeps
  the step, with a note.
- ghalint has no Action of its own, so only shell steps move. `brew install ghalint`, `brew install
  suzuki-shunsuke/ghalint/ghalint`, and `go install github.com/suzuki-shunsuke/ghalint/cmd/ghalint@...` install it.
  `ghalint run` becomes the crosgha Action, and `ghalint run-action` (or `act`) with files becomes one with those
  files as `inputs`. Without files it checks every action file up to three directories deep, so the step stays with a
  note, as do `-c`/`-config`, a `GHALINT_CONFIG` in the step's `env`, and `ghalint exp`.
- For octoscan, `synacktiv/action-octoscan` steps become crosgha Action steps with `fail-on: none`, since that Action
  accepts octoscan's findings exit status, and a `workdir` other than `.` becomes `inputs`. Its SARIF upload is
  handled as for poutine: an upload of `steps.<id>.outputs.sarif_output` (or of `octoscan.sarif`) is removed, and
  without one the step gets `advanced-security: 'false'`. `version`, `token`, `filter_triggers`, `filter_run`, and
  `debug_rules` are dropped: they pick the octoscan release or tune octoscan's own rules. A step with `ignore`,
  `disable_rules`, `enable_rules`, or `config_file`, or whose report a later step reads, stays, with a note. In shell
  steps, `docker pull ghcr.io/synacktiv/octoscan[:tag]` installs it and `octoscan scan <target>` becomes the crosgha
  Action with the target as `inputs`, failing on findings as octoscan does. `-d`, `--debug`, `--verbose`,
  `--oneline`, `--filter-run`, `--debug-rules`, and `--filter-triggers` are dropped; `octoscan dl`, `--format`, and
  any other flag keep the step, with a note. octoscan reads no config file on its own, so none is migrated.
- Shell steps that only install the tool are removed only when a crosgha step replaced the tool in the same job and no
  step of that job still runs it; otherwise they stay with a note. Steps that only run it become crosgha Action steps,
  with CLI flags translated to inputs. Steps that install or run the tool next to other commands, with shell syntax in
  their arguments (`;`, redirects, pipes, `$(...)`, `$VAR`), or with a flag the Action has no input for (such as
  `--format json`), stay in place and are listed for a manual change.
- Config migration turns `zizmor.yml` into `crosgha.yml` next to it, with rule keys renamed where the match is one to
  one and keys without a crosgha rule dropped. A key whose new name is already set keeps its zizmor name, with a note.
  `actionlint.yaml` self-hosted runner labels become `rules.self-hosted-runner.config.labels`. poutine's config
  (`.poutine.yml` or `.github/poutine.yml`) has no crosgha meaning: its `skip` and `include` entries are listed in
  notes, and no `crosgha.yml` is written. The same goes for ghalint's `excludes` (`ghalint.yaml`, `.ghalint.yaml`, or
  `.github/ghalint.yaml`). The original config stays in place.

The command writes the latest crosgha release, pinned by SHA with a version comment. Without a token, pass the
reference yourself with `--action owner/repo@<sha> --action-version vX.Y.Z`. A symlinked workflow that needs an edit
stops the command before any file is written; one that needs none is left alone.
