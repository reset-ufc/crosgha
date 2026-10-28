# Usage

## Scanning

Each `PATH` may be a workflow file, a directory of workflows, a repository root, or its `.github`; the last two are
scanned through `.github/workflows` as zizmor does with `.`. Subdirectories are never scanned: GitHub does not load
workflows from them. Without a `PATH`, crosgha scans `.github/workflows`. A `dependabot.yml` path gets only the
Dependabot cooldown check. Symlinked files are scanned but never rewritten; a symlink to another scanned workflow is
reported and fixed once, through its target.

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
