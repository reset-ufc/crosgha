# Getting started

This walkthrough takes one realistic CI workflow, finds what crosgha reports on it at each persona, applies the fixes
that pass validation, fixes the rest by hand, and ends with the workflow clean, checked in CI and before every push.

## 1. Install

On Linux x86_64, the install script puts the latest release in `~/.local/bin` after checking it against the release's
`sha256.sum` ([Install](install.md)):

```sh
curl -fsSL https://raw.githubusercontent.com/reset-ufc/crosgha/master/install.sh | sh
```

`CROSGHA_VERSION=X.Y.Z` pins a release and `CROSGHA_INSTALL_DIR` changes the target directory. Release archives and
building from source are on the same page.

## 2. Take a workflow

Save this as `.github/workflows/ci.yml`. It is a Python CI job with the shape most repositories have: a checkout, a
setup step, a cache, an install, a test run, an artifact upload, and one step that prints the pull request title.

```yaml
name: CI

on:
  pull_request:

jobs:
  test:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v7
      - uses: actions/setup-python@v7
        with:
          python-version: "3.13"
      - uses: actions/cache@v6
        with:
          path: ~/.cache/pip
          key: pip-${{ hashFiles('requirements.txt') }}
      - run: pip install -r requirements.txt
      - name: Print the pull request title
        run: echo "${{ github.event.pull_request.title }}"
      - run: pytest --junitxml=coverage/report.xml
      - uses: actions/upload-artifact@v7
        with:
          name: test-report
          path: coverage/
```

## 3. First scan

Run it with the default persona. `--offline` keeps the run off the GitHub API, so it takes no token:

```console
$ crosgha --offline .github/workflows/ci.yml
warning[persist-credentials]: checkout retains credentials for later steps
  --> .github/workflows/ci.yml:10:9
   |
10 |       - uses: actions/checkout@v7
   |         ^^^^ job `test`, step 1
   |
   = help: actions/checkout must not leave the token on disk for later steps; there is no automatic fix when a later step pushes with it, `true` is explicit, or the checkout predates v2.
   = note: same as zizmor: artipacked

error[script-injection]: untrusted `github.event.pull_request.title` expands into a script
  --> .github/workflows/ci.yml:20:9
   |
20 |         run: echo "${{ github.event.pull_request.title }}"
   |         ^^^ job `test`, step 5
   |
   = help: Untrusted event data and composite action inputs must not be expanded into scripts with `${{ }}`.
   = note: same as zizmor: template-injection; actionlint: expression; ghlint: ShellScriptInjection, JSScriptInjection

Checked 1 workflow file: 2 findings (1 error, 1 warning).
1 finding has a fix: rerun with --fix.
```

The default persona reports the security problems worth acting on, and here it finds two. The run exits with status 1,
because `--fail-on` defaults to `any`.

## 4. Read a finding

Take the first block. Its parts carry the whole message:

| Piece | In the example | Meaning |
|---|---|---|
| `warning[persist-credentials]` | severity and rule ID | Which rule fired, and how serious it is: `note`, `warning`, or `error` |
| `checkout retains credentials for later steps` | message | What is wrong, in one line |
| `.github/workflows/ci.yml:10:9` | path, line, column | Where the node the rule read starts |
| `- uses: actions/checkout@v7` with `^^^^` under `uses` | source | The exact node, underlined |
| `job \`test\`, step 1` | location | The job and step that own the node |
| `= help:` | help | What the rule asks for |
| `= note: same as zizmor: artipacked` | equivalent | The same concept in another tool, so existing ignores keep their names |

The last two lines are the summary: how many findings, which files, and how many of them have a fix. The rule IDs, what
each one means, and where each comes from are in the [rule catalog](rules.md), with a non-compliant and a compliant
example per rule in [Rule examples](rule-examples.md).

## 5. Raise the persona

The default persona shows security problems. `--persona pedantic` adds the code smells and hardening policies:
unpinned actions, missing permissions, missing timeouts, trigger filters, concurrency.

```console
$ crosgha --offline --persona pedantic .github/workflows/ci.yml
note[missing-concurrency]: superseded runs keep running without a concurrency group
 --> .github/workflows/ci.yml:3:1
  |
3 | on:
  | ^^ workflow
  |
  = help: CI workflows should cancel superseded runs with a concurrency group.
  = note: same as zizmor: concurrency-limits

note[broad-trigger]: `pull_request` has no branch, tag, or path filter
 --> .github/workflows/ci.yml:4:3
  |
4 |   pull_request:
  |   ^^^^^^^^^^^^ workflow
  |
  = help: push and pull_request triggers should filter branches, tags, or paths.

warning[missing-permissions-block]: job relies on default GITHUB_TOKEN permissions
 --> .github/workflows/ci.yml:7:3
  |
7 |   test:
  |   ^^^^ job `test`
  |
  = help: Jobs must declare GITHUB_TOKEN permissions instead of relying on defaults.
  = note: same as ghlint: MissingJobPermissions

note[missing-timeout]: executable job has no timeout-minutes
 --> .github/workflows/ci.yml:7:3
  |
7 |   test:
  |   ^^^^ job `test`
  |
  = help: Jobs should set `timeout-minutes`.
  = note: same as ghlint: MissingJobTimeout

warning[persist-credentials]: checkout retains credentials for later steps
  --> .github/workflows/ci.yml:10:9
   |
10 |       - uses: actions/checkout@v7
   |         ^^^^ job `test`, step 1
   |
   = help: actions/checkout must not leave the token on disk for later steps; there is no automatic fix when a later step pushes with it, `true` is explicit, or the checkout predates v2.
   = note: same as zizmor: artipacked

warning[unpinned-action]: remote action or workflow is not pinned to a full commit SHA
  --> .github/workflows/ci.yml:10:9
   |
10 |       - uses: actions/checkout@v7
   |         ^^^^ job `test`, step 1
   |
   = help: Actions, reusable workflows, and container images must be pinned to a full commit SHA or image digest.
   = note: same as zizmor: unpinned-uses, unpinned-images

...

note[cache-misuse]: setup action has a built-in `cache` input for this
  --> .github/workflows/ci.yml:14:9
   |
14 |       - uses: actions/cache@v6
   |         ^^^^ job `test`, step 3
   |
   = help: Use the setup action's `cache` input instead of a separate actions/cache step.

error[script-injection]: untrusted `github.event.pull_request.title` expands into a script
  --> .github/workflows/ci.yml:20:9
   |
20 |         run: echo "${{ github.event.pull_request.title }}"
   |         ^^^ job `test`, step 5
   |
   = help: Untrusted event data and composite action inputs must not be expanded into scripts with `${{ }}`.
   = note: same as zizmor: template-injection; actionlint: expression; ghlint: ShellScriptInjection, JSScriptInjection

note[error-suppression]: upload-artifact passes when `path` matches no files; set `if-no-files-found: error`
  --> .github/workflows/ci.yml:22:9
   |
22 |       - uses: actions/upload-artifact@v7
   |         ^^^^ job `test`, step 7
   |
   = help: `continue-on-error: true` hides failures, and so do actions that pass when they find nothing to upload, attach, or publish; create-pull-request and versions before the fail-fast input have no automatic fix.
   = note: same as ghlint: FailFastUploadArtifact, FailFastSoftpropsGhRelease, FailFastPublishUnitTestResults, FailFastPeterEvansCreatePullRequest

...

Checked 1 workflow file: 12 findings (1 error, 6 warnings, 5 notes).
2 findings have a fix: rerun with --fix.
```

The `...` lines stand for the `unpinned-action` finding on the other three `uses:` lines, at lines 11, 14, and 22.
Two of the twelve findings have a fix: `persist-credentials` and `error-suppression`. Pinning an action, choosing a
timeout or a permission set, and deciding how to pass untrusted text are all decisions crosgha cannot make for you, so
the other ten stay findings.

## 6. Apply the fixes crosgha can prove

```console
$ cp .github/workflows/ci.yml /tmp/ci.yml.orig
$ crosgha --offline --persona pedantic --fix .github/workflows/ci.yml
note[missing-concurrency]: superseded runs keep running without a concurrency group
 --> .github/workflows/ci.yml:3:1
  |
3 | on:
  | ^^ workflow
  |
  = help: CI workflows should cancel superseded runs with a concurrency group.
  = note: same as zizmor: concurrency-limits

note[broad-trigger]: `pull_request` has no branch, tag, or path filter
 --> .github/workflows/ci.yml:4:3
  |
4 |   pull_request:
  |   ^^^^^^^^^^^^ workflow
  |
  = help: push and pull_request triggers should filter branches, tags, or paths.

warning[missing-permissions-block]: job relies on default GITHUB_TOKEN permissions
 --> .github/workflows/ci.yml:7:3
  |
7 |   test:
  |   ^^^^ job `test`
  |
  = help: Jobs must declare GITHUB_TOKEN permissions instead of relying on defaults.
  = note: same as ghlint: MissingJobPermissions

note[missing-timeout]: executable job has no timeout-minutes
 --> .github/workflows/ci.yml:7:3
  |
7 |   test:
  |   ^^^^ job `test`
  |
  = help: Jobs should set `timeout-minutes`.
  = note: same as ghlint: MissingJobTimeout

warning[unpinned-action]: remote action or workflow is not pinned to a full commit SHA
  --> .github/workflows/ci.yml:10:9
   |
10 |       - uses: actions/checkout@v7
   |         ^^^^ job `test`, step 1
   |
   = help: Actions, reusable workflows, and container images must be pinned to a full commit SHA or image digest.
   = note: same as zizmor: unpinned-uses, unpinned-images

...

error[script-injection]: untrusted `github.event.pull_request.title` expands into a script
  --> .github/workflows/ci.yml:22:9
   |
22 |         run: echo "${{ github.event.pull_request.title }}"
   |         ^^^ job `test`, step 5
   |
   = help: Untrusted event data and composite action inputs must not be expanded into scripts with `${{ }}`.
   = note: same as zizmor: template-injection; actionlint: expression; ghlint: ShellScriptInjection, JSScriptInjection

...

Fixed 2 findings in 1 workflow file.
Remaining: 10 findings (1 error, 5 warnings, 4 notes).
```

The `...` lines stand for the `unpinned-action` finding on the other three `uses:` lines and for `cache-misuse`, none
of which crosgha can decide for you. `--fix` rewrites only what it can prove: the patched file must re-parse, the
targeted finding must be gone with no new finding in its place, and only the intended YAML node may change. Here is
what it changed, compared with the copy saved before the run:

```console
$ diff -u /tmp/ci.yml.orig .github/workflows/ci.yml
--- /tmp/ci.yml.orig	2026-11-24 13:46:22.781457836 -0300
+++ .github/workflows/ci.yml	2026-11-24 13:46:22.817126221 -0300
@@ -8,6 +8,8 @@
     runs-on: ubuntu-24.04
     steps:
       - uses: actions/checkout@v7
+        with:
+          persist-credentials: false
       - uses: actions/setup-python@v7
         with:
           python-version: "3.13"
@@ -23,3 +25,4 @@
         with:
           name: test-report
           path: coverage/
+          if-no-files-found: error
```

The fixes add `persist-credentials: false` and `if-no-files-found: error`, creating `with` where needed, and leave
every other byte, comment, and blank line alone. Which rules have a fix and what
each one leaves unfixed: [Fixes](fixes.md).

## 7. Fix the rest by hand

The remaining ten findings need your judgment. This version pins every action to a full commit SHA with its version
in a comment, declares `permissions` and `timeout-minutes`, filters the trigger, adds a `concurrency` group, replaces
the separate cache step with `setup-python`'s `cache` input, and passes the pull request title through `env` so the
shell never parses it:

```yaml
name: CI

on:
  pull_request:
    branches: [main]

permissions:
  contents: read

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  test:
    runs-on: ubuntu-24.04
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          persist-credentials: false
      - uses: actions/setup-python@6d4fd22b20465ccc0f7354476588e8df7c9ca0e0 # v7.1.0
        with:
          python-version: "3.13"
          cache: pip
      - run: pip install -r requirements.txt
      - name: Print the pull request title
        run: echo "$TITLE"
        env:
          TITLE: ${{ github.event.pull_request.title }}
      - run: pytest --junitxml=coverage/report.xml
      - uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2
        with:
          name: test-report
          path: coverage/
          if-no-files-found: error
```

```console
$ crosgha --offline --persona pedantic .github/workflows/ci.yml
✓ Checked 1 workflow file: no findings.
```

Exit status 0. The same file at the `auditor` persona reports 8 more notes: a single runner and toolchain, the unnamed
job and `run` steps, the missing `shell`, and the missing runtime monitor. The `auditor` persona is where the
heuristics that may be false positives live, so read them before acting.

## 8. Run it in CI

The [crosgha Action](action.md) runs the same binary on every pull request and uploads SARIF to code scanning, so the
findings also appear in the Security tab. There is no moving `v1` tag: pin the Action by full commit SHA with a
version comment.

```yaml
name: crosgha

on:
  pull_request:
    branches: [main]

permissions:
  contents: read
  security-events: write # only for the SARIF upload (advanced-security: true, the default)

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  crosgha:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@<sha> # vX.Y.Z
        with:
          persist-credentials: false
      - uses: reset-ufc/crosgha@<sha> # vX.Y.Z
        with:
          persona: pedantic
```

Replace both `<sha>` and `<vX.Y.Z>` with the commit and tag of the release you pin. With a full SHA in each `uses:`
line, this workflow is reported clean at `--persona pedantic`. The Action downloads the release that matches its own
revision, verifies it against the release's `sha256.sum`, runs crosgha once, uploads SARIF, and only then fails
according to `fail-on`; it accepts zizmor-action's inputs, so a zizmor step needs only its `uses:` line changed. The
full input list, the `fix: pull-request` mode, and auto-merge are in [GitHub Action](action.md).

## 9. Catch it before the push

The pre-push hook runs the CLI at the repository root before every push, on the working tree, so uncommitted edits
count. `install` writes to the hooks directory git resolves for the repository, which is the path it prints:

```console
$ crosgha hook install -- --offline --persona pedantic
Installed /tmp/opencode/acme-site/.git/hooks/pre-push.
```

Everything after `--` is validated and passed to every scan, and re-running `install` replaces those arguments. On a
finding the hook blocks the push and tells you to run `crosgha --fix`; it never edits files itself. `crosgha hook
uninstall` removes it, and both commands refuse to touch a `pre-push` hook that crosgha did not write. More in
[Usage](usage.md#pre-push-hook).

## Where next

- What each of the 51 rules checks, and its equivalent in another tool: [Rules](rules.md).
- Personas, `crosgha.yml` and `zizmor.yml`, ignoring a finding, severity remaps: [Configuration](configuration.md).
- Which paths and actions are scanned, remote repositories, migrating from another tool: [Usage](usage.md).
- What a fix proves before it is written: [Fixes](fixes.md) and [How crosgha works](how-it-works.md#how-a-fix-is-accepted).
