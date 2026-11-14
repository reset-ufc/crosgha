# Usage

## Scanning

Each `PATH` may be a workflow file, an action file, a directory of workflows, a repository root, or its `.github`; the
last two are scanned through `.github/workflows` as zizmor does with `.`. Subdirectories are never scanned: GitHub does
not load workflows from them. Without a `PATH`, crosgha scans `.github/workflows`. A `dependabot.yml` path gets only the
Dependabot cooldown check. Symlinked files are scanned but never rewritten; a symlink to another scanned workflow or
action is reported and fixed once, through its target.

### Actions

A file named `action.yml` or `action.yaml` is an [action metadata
file](https://docs.github.com/en/actions/reference/workflows-and-actions/metadata-syntax), unless it sits in
`.github/workflows`, where it is a workflow (zizmor tells them apart the same way).

- **Named as a `PATH`:** always scanned, and so is an action file directly inside a directory `PATH`. In a directory,
  `action.yaml` is skipped when `action.yml` is next to it, because the runner then never reads it.
- **With a repository:** a repository root, its `.github`, or its `.github/workflows` (also the default `PATH`) adds the
  actions the repository runs or publishes: the root `action.yml` (or `action.yaml`), and each local action a scanned
  workflow calls with `uses: ./path`, followed through the composite actions it calls in turn. Paths resolve from the
  repository root, the caller's checkout. Each action is scanned once, a cycle included; a `uses: ./path` without a
  metadata file (a missing directory, or a Docker action with only a `Dockerfile`) is skipped. crosgha does not walk
  the other directories as zizmor does, so an action that no workflow calls, such as one published from a
  subdirectory, is scanned only when given as a `PATH`. A repository with a root action and no workflows is not an
  empty input.
- **What runs:** a composite action's `runs.steps` get every rule that reads steps, `uses` lines, or text anywhere in
  the file; JavaScript and Docker actions have no steps, so only those file-wide checks and the metadata checks apply.
  Rules that read triggers, jobs, `permissions`, timeouts, concurrency, or `defaults` never run on an action. The
  **Actions** column of the [rule catalog](rules.md) lists what each rule checks, and `--list-rules` prints it last.
- **Inputs are untrusted:** in a composite action, `script-injection` also reports `${{ inputs.<name> }}` expanded into
  a `run` script or `actions/github-script`'s `script`, since the caller sets every input, possibly from event data.
  Pass the input through `env` instead.
- **Invalid metadata:** an action without a `runs` mapping or a string `runs.using` cannot be parsed, like a workflow
  without `on` or `jobs`: it is reported on stderr, the other files are still checked, the run exits 2, and `--fix`
  writes nothing. Other schema problems, such as a composite `run` step without `shell`, are `schema-validity`
  findings.
- **Fixes, ignores, and config:** the same as for workflows: ignore comments in the action file, `crosgha.yml` or
  `zizmor.yml` of the input (an action found through a repository uses that repository's), and the verified fixes.
  `persist-credentials` is never fixed in an action, because the steps that run after it are in the caller's workflow.
- **Remote inputs** read workflows only; their repository's actions are tracked in #99, and the actions they call
  from other repositories in #100.

`crosgha owner/repo@ref` scans workflows directly in that GitHub repository's `.github/workflows`.
Omit `@ref` to scan the default branch. A local path with the same spelling takes precedence.
`https://github.com/owner/repo@ref` is also accepted. The report names the resolved commit SHA, so
all files come from one revision. Remote inputs require `--gh-token`, `GH_TOKEN`, or `GITHUB_TOKEN`;
private repositories outside the caller's repository need a token that can read them. The scan
makes one commit request, one directory request, and one request per YAML workflow before online
rules. `--no-online-audits` skips those rules but still fetches the workflows. `--offline` and
`--fix` reject remote inputs. Repository rules and remote config discovery are tracked in #99.

## Pre-push hook

```sh
crosgha hook install -- --offline --persona pedantic
crosgha hook uninstall
```

`install` writes a git `pre-push` hook in the directory git resolves for hooks, so `core.hooksPath` is respected. It
refuses a hooks directory outside the repository, such as a global `core.hooksPath`, where the hook would run on every
push from every repository.

- **Arguments:** everything after `--` is validated and passed to every scan. Re-running `install` replaces them.
  Subcommands, `--fix`, `--list-rules`, `--version`, and `--help` are refused: the hook only scans.
- **What it runs:** `crosgha` from `PATH`, at the repository root, on the working tree (uncommitted edits count).
- **On findings:** it blocks the push and tells you to run `crosgha --fix`. It never edits files itself.
- **Safety:** both commands refuse to touch a `pre-push` hook that crosgha did not write. A hook counts as crosgha's
  only if it matches the generated script exactly, so an edited copy is yours.

## Migrating from zizmor or actionlint

`crosgha migrate --from zizmor|actionlint [PATH]` moves a repository over:

- **Action steps:** `zizmorcore/zizmor-action`, `raven-actions/actionlint`, `reviewdog/action-actionlint`, and
  `rhysd/actionlint` steps become crosgha Action steps. zizmor steps keep their `with:` block (the crosgha Action
  accepts zizmor-action's inputs); actionlint steps lose theirs.
- **Shell steps:** steps that only install the tool are removed only when a crosgha step replaced the tool in the same
  job and no step of that job still runs it; otherwise they stay with a note. Steps that only run it become crosgha
  Action steps, with CLI flags translated to inputs. Steps that install or run the tool next to other commands, with
  shell syntax in their arguments (`;`, redirects, pipes, `$(...)`, `$VAR`), or with a flag the Action has no input
  for (such as `--format json`), stay in place and are listed for a manual change.
- **Config:** `zizmor.yml` becomes `crosgha.yml` next to it, with rule keys renamed where the match is one to one and
  keys without a crosgha rule dropped. A key whose new name is already set keeps its zizmor name, with a note.
  `actionlint.yaml` self-hosted runner labels become `rules.self-hosted-runner.config.labels`. The original config
  stays in place.

The command writes the latest crosgha release, pinned by SHA with a version comment. Without a token, pass the
reference yourself with `--action owner/repo@<sha> --action-version vX.Y.Z`. A symlinked workflow that needs an edit
stops the command before any file is written; one that needs none is left alone.
