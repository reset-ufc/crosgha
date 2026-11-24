# Supply chain

These rules follow the code a workflow pulls in before it runs: which package version, which downloaded script,
which action, which commit, and which action repository. Five read one file, five ask the GitHub API, and one reads
the Dependabot config next to the workflows. None of them has a fix.

## `unpinned-package`

Tools installed in run steps must name an exact version.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | pedantic | workflow | none | zizmor: `adhoc-packages`, `unpinned-tools` |

**What it reports.** It reads every `run` step and reports the first command that installs or runs a package without an
exact version. `pip`, `pip3`, `pipx`, and `uv pip install` need `==`; `uv tool install` needs `==` or a `name@version`
other than `@latest`; `npm`, `pnpm`, and `yarn global add` need `major.minor.patch` after `@`, so `@1`, `@1.x`,
`@^1.2.0`, `@*`, `@next`, and `@latest` all fail; `go install` needs `vX.Y.Z` or at least seven hexadecimal digits;
conda, mamba, and micromamba need `=` and a `<` or `>` range fails; `cargo install` and `gem install` need their
version option or `name@version` / `name:version`. Commands that download and run a tool in one step are checked
with the rule of their package manager: `go run module@version` like `go install` (without `@version` it builds
from the module that `go.sum` pins and is not reported), `npx` and `pnpm dlx`/`pnpx` like npm, `uvx` and
`uv tool run` like `uv tool install`, and `pipx run` through `--spec`. A bare `npx eslint` may run the project's own
locked binary, so `npx` is reported only with an explicit moving version such as `@latest` or `@9`; only the package
operand counts, never the arguments passed to the tool. Local paths, URLs, archives, shell variables, and `${{ }}`
operands are skipped, and `npm install` or `npm ci` with no package name lists nothing to check. At most one finding
is reported per step, from
[`src/rules/supply_chain.rs`](https://github.com/reset-ufc/crosgha/blob/master/src/rules/supply_chain.rs).

**Why it matters.** An install without a version takes whatever the registry serves at run time, so a compromised
release runs in the job without a change to the workflow. Khatami et al. (SCAM 2024) list it as KZ-20 "Specify
package versions", and zizmor's [`adhoc-packages` and `unpinned-tools`](https://docs.zizmor.sh/audits/) audits cover
the same practice.

**Example.**

```yaml
name: Tools
on:
  push:
    branches: [main]
concurrency:
  group: tools-${{ github.ref }}
permissions: {}
jobs:
  tools:
    name: Install tools
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Install TypeScript
        shell: bash
        run: npm install -g typescript
```

```text
warning[unpinned-package]: `npm install -g typescript` fetches a package without an exact version
  --> .github/workflows/example.yml:16:9
   |
16 |         run: npm install -g typescript
   |         ^^^ job `tools`, step 1
   |
   = help: Tools installed in run steps must name an exact version.
   = note: same as zizmor: adhoc-packages, unpinned-tools

Checked 1 workflow file: 2 findings (1 warning, 1 note).
```

Compliant:

```yaml
name: Tools
on:
  push:
    branches: [main]
concurrency:
  group: tools-${{ github.ref }}
permissions: {}
jobs:
  tools:
    name: Install tools
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Install TypeScript
        shell: bash
        run: npm install -g typescript@5.6.2
```

**How to fix.** Add the exact version the registry publishes, in the syntax of that ecosystem (`typescript@5.6.2`,
`requests==2.32.3`, `ruff@0.6.0`, `cargo-deny --version 0.16.1`). A lockfile installed with `pip install -r`,
`npm ci`, or `micromamba install --file` is already exact and is not reported. `--fix` does not rewrite it. Ignoring
a finding works through comments or config; see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#unpinned-package).

## `unverified-remote-script`

Downloaded scripts must not run before their integrity is checked.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | none | none |

**What it reports.** A `run` step is reported when a download reaches an interpreter that runs its standard input:
`curl … | sh`, `wget … | sudo bash`, `bash <(curl …)`, or `$(curl …)` inside `sh -c`, `bash -c`, or `eval`. It is
also reported when a file saved by `curl -o`, `curl -O`, or `wget -O` is later run by an interpreter, by `source` or
`.`, or as a path, and no `sha256sum`, `sha512sum`, `shasum`, `gpg`, `cosign`, or `minisign` ran first. Shell
comments are removed before the check, and a heredoc that a shell reads is treated as a script. A pipe into `jq`,
`python -c`, `perl -ne`, or `bash -c` feeds data to an argument and is not reported.

**Why it matters.** The bytes that run are chosen by the server at request time, not by the repository under review,
so a compromised host or a swapped file executes with the job's token. Poutine's `unverified_script_exec` and
Semgrep's `curl-eval` check the same pattern.

**Example.**

```yaml
name: Install
on:
  push:
    branches: [main]
concurrency:
  group: install-${{ github.ref }}
permissions: {}
jobs:
  install:
    name: Install tool
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Run the installer
        shell: bash
        run: curl -fsSL https://example.com/install.sh | sh
```

```text
warning[unverified-remote-script]: downloaded script runs without an integrity check
  --> .github/workflows/example.yml:16:9
   |
16 |         run: curl -fsSL https://example.com/install.sh | sh
   |         ^^^ job `install`, step 1
   |
   = help: Downloaded scripts must not run before their integrity is checked.

Checked 1 workflow file: 2 findings (1 warning, 1 note).
```

Compliant:

```yaml
name: Install
on:
  push:
    branches: [main]
concurrency:
  group: install-${{ github.ref }}
permissions: {}
jobs:
  install:
    name: Install tool
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Run the installer
        shell: bash
        run: |
          curl -fsSLo install.sh https://example.com/install.sh
          sha256sum -c install.sh.sha256
          sh install.sh
```

**How to fix.** Save the download to a file, check it against a published digest or signature, and only then run it,
as above; the digest file must come from the same release, not from the same request as the script. `--fix` does not
rewrite it. Ignoring a finding works through comments or config; see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#unverified-remote-script).

## `typosquatted-action`

Action owners one edit away from a well-known publisher may be typosquats.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| error | regular | workflow | none | zizmor: `typosquat-uses` |

**What it reports.** It takes the owner of every remote `uses`, in a step or at job level, and compares it, ignoring
case, with twelve high-traffic publishers: `actions`, `github`, `docker`, `aws-actions`, `azure`,
`google-github-actions`, `hashicorp`, `gradle`, `pypa`, `codecov`, `step-security`, and `anthropics`. An owner one
insertion, deletion, substitution, or adjacent transposition away is reported. `actionsx` is a known legitimate
neighbour and is skipped. Local actions, `docker://` images, and values containing `${{ }}` are not checked, and the
rule never asks the API whether the owner exists.

**Why it matters.** One wrong character in an owner name installs code from a repository its author never audited,
and a pinned SHA makes the line look as trustworthy as the real one. zizmor's
[`typosquat-uses`](https://docs.zizmor.sh/audits/) audit covers the same shape.

**Example.**

```yaml
name: Checkout
on:
  push:
    branches: [main]
concurrency:
  group: checkout-${{ github.ref }}
permissions: {}
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Check out the repository
        uses: actons/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

```text
error[typosquatted-action]: owner `actons` is one edit away from `actions`
  --> .github/workflows/example.yml:15:9
   |
15 |         uses: actons/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
   |         ^^^^ job `build`, step 1
   |
   = help: Action owners one edit away from a well-known publisher may be typosquats.
   = note: taxonomy tag `typosquatting/impostor-commit`; same as zizmor: typosquat-uses

Checked 1 workflow file: 2 findings (1 error, 1 note).
```

Compliant:

```yaml
name: Checkout
on:
  push:
    branches: [main]
concurrency:
  group: checkout-${{ github.ref }}
permissions: {}
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Check out the repository
        uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with: { persist-credentials: false }
```

**How to fix.** Correct the owner to the publisher you meant, then confirm the SHA belongs to that repository, which
[`impostor-commit`](#impostor-commit) checks. `--fix` does not rewrite it. Ignoring a finding works through comments
or config; see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#typosquatted-action).

## `unpinned-action`

Actions, reusable workflows, and container images must be pinned to a full commit SHA or image digest.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | pedantic | workflow | none | zizmor: `unpinned-uses`, `unpinned-images` |

**What it reports.** It checks every `uses` in a step and at job level, a job's `container` or `container.image`,
each `services.<name>.image`, and, in an action file, a Docker `runs.image` that starts with `docker://`. A remote
reference passes only with 40 hexadecimal characters after `@`, and an image only with a `sha256:` digest of 64 hex
characters or a `sha512:` digest of 128. Local paths (`./`, `$/`), expressions, and references covered by a `ref-pin`
or `any` policy are skipped. It reads syntax only: whether the commit exists, and whose repository it belongs to, is
left to [`impostor-commit`](#impostor-commit)
([`src/rules/immutable_references.rs`](https://github.com/reset-ufc/crosgha/blob/master/src/rules/immutable_references.rs)).

**Why it matters.** GitHub asks you to "Pin references to actions to a commit SHA in your workflow files", and warns
that "a tag can be moved or deleted if a bad actor gains access to the repository storing the action"
([secure-use.md:145](https://github.com/github/docs/blob/44f009da482ef1470b37ffea456821014d892a1f/content/actions/reference/security/secure-use.md#L145),
[secure-use.md:151](https://github.com/github/docs/blob/44f009da482ef1470b37ffea456821014d892a1f/content/actions/reference/security/secure-use.md#L151)),
which [the GitHub guidance page](../github-guidance.md#secrets-pinning-and-the-actions-you-trust) maps to this rule.

**Example.**

```yaml
name: Pin
on:
  push:
    branches: [main]
concurrency:
  group: pin-${{ github.ref }}
permissions: {}
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Set up the toolchain
        uses: example/setup@v2
```

```text
warning[unpinned-action]: remote action or workflow is not pinned to a full commit SHA
  --> .github/workflows/example.yml:15:9
   |
15 |         uses: example/setup@v2
   |         ^^^^ job `build`, step 1
   |
   = help: Actions, reusable workflows, and container images must be pinned to a full commit SHA or image digest.
   = note: same as zizmor: unpinned-uses, unpinned-images

Checked 1 workflow file: 2 findings (1 warning, 1 note).
```

Compliant:

```yaml
name: Pin
on:
  push:
    branches: [main]
concurrency:
  group: pin-${{ github.ref }}
permissions: {}
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Set up the toolchain
        uses: example/setup@cccccccccccccccccccccccccccccccccccccccc
```

**How to fix.** Replace the tag or branch with the full SHA of the release you reviewed, and keep the release name in
a comment next to it so the line stays readable; for an image, use `name@sha256:<digest>` from the registry. `--fix`
does not rewrite it. Ignoring a finding works through comments or config; see
[ignoring findings](../configuration.md#ignoring-findings).

**Options.** `rules.unpinned-action.config.policies` maps repository patterns to `hash-pin`, `ref-pin`, or `any`; an
unmatched reference gets the implicit `*: hash-pin`, and the most specific matching pattern wins. Keys may use
crosgha's or zizmor's rule names ([config file](../configuration.md#config-file)).

```yaml
rules:
  unpinned-action:
    config:
      policies:
        actions/*: ref-pin
        github/codeql-action/init: hash-pin
        example/trusted: any
```

All fixtures for this rule: [examples](../rule-examples.md#unpinned-action).

## `forbidden-action`

`uses` must match the configured allowlist or denylist.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | workflow | none | zizmor: `forbidden-uses` |

**What it reports.** It does nothing until `allow` or `deny` is configured. A `uses` that matches a `deny` pattern is
reported, and when `allow` is set, every `uses` that matches none of its patterns is reported too. Patterns follow
zizmor's repository patterns: `*`, `owner/*`, `owner/repo/*`, `owner/repo`, `owner/repo/path`, and a whole
`uses@ref`, compared without case. Local paths and `docker://` images are never reported.

**Why it matters.** A denylist keeps the repositories you decided not to trust out of every workflow, including the
ones nobody reviews again, and an allowlist makes a new third-party action a deliberate decision. The rule follows
zizmor's [`forbidden-uses`](https://docs.zizmor.sh/audits/#forbidden-uses) audit and its pattern syntax.

**Example.** With this `crosgha.yml`:

```yaml
rules:
  forbidden-action:
    config:
      deny:
        - tj-actions/*
```

```yaml
name: Changed files
on:
  push:
    branches: [main]
concurrency:
  group: changed-${{ github.ref }}
permissions: {}
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: List changed files
        uses: tj-actions/changed-files@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

```text
warning[forbidden-action]: `tj-actions/changed-files@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa` is not allowed by the forbidden-uses policy
  --> .github/workflows/example.yml:15:9
   |
15 |         uses: tj-actions/changed-files@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
   |         ^^^^ job `build`, step 1
   |
   = help: `uses` must match the configured allowlist or denylist.
   = note: taxonomy tag `untrusted-3rd-party-action`; same as zizmor: forbidden-uses

Checked 1 workflow file: 2 findings (1 warning, 1 note).
```

Compliant:

```yaml
name: Changed files
on:
  push:
    branches: [main]
concurrency:
  group: changed-${{ github.ref }}
permissions: {}
jobs:
  build:
    name: Build
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: List changed files
        uses: example/changed-files@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

**How to fix.** Replace the action with one your policy allows, or widen the policy when the action is a deliberate
choice. `--fix` does not rewrite it. Ignoring a finding works through comments or config; see
[ignoring findings](../configuration.md#ignoring-findings).

**Options.** `rules.forbidden-action.config.allow` and `.deny` take repository patterns; `--config` or a discovered
`crosgha.yml` supplies them, and keys may use zizmor's rule name ([config file](../configuration.md#config-file)).

```yaml
rules:
  forbidden-action:
    config:
      allow:
        - actions/*
        - github/codeql-action/*
```

All fixtures for this rule: [examples](../rule-examples.md#forbidden-action).

## `known-vulnerable-action`

Referenced action versions must not have published advisories.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| error | regular | online | none | zizmor: `known-vulnerable-actions` |

**What it reports.** For every remote `uses` that names a version, it asks the GitHub Advisory Database for
advisories that affect that action at that version, and reports each `GHSA` ID unless `allow` lists it. The version
is the tag in the reference, or the first word of the `# vX.Y.Z` comment after a pinned SHA. A partial version such
as `@v4` is first resolved to the full release tag on the same commit, because the advisories API reads `@4` as
`4.0.0`. A reference with no version, and a lookup that fails, produce nothing
([`src/rules/online.rs`](https://github.com/reset-ufc/crosgha/blob/master/src/rules/online.rs)).

**Why it matters.** The rule reads the
[GitHub Advisory Database](https://docs.github.com/en/rest/security-advisories/global-advisories) for the `actions`
ecosystem, which records published vulnerabilities in the actions you call. GitHub also notes that Dependabot "will
not create alerts for actions pinned to SHA values"
([secure-use.md:336](https://github.com/github/docs/blob/44f009da482ef1470b37ffea456821014d892a1f/content/actions/reference/security/secure-use.md#L336)),
so a pinned workflow gets no alert without a check like this one.

**Example.**

```yaml
name: Changed files
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: tj-actions/changed-files@v35
```

This rule queries the GitHub API, so it is skipped with `--offline`; the test fixture
[`known-vulnerable-action.before.yml`](https://github.com/reset-ufc/crosgha/blob/master/tests/fixtures/online/known-vulnerable-action.before.yml)
and its fake API answers show the two findings it reports for `@v35` and `@v4.0.0`.

Compliant:

```yaml
name: Changed files
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: tj-actions/changed-files@v46.0.5
```

**How to fix.** Move to a release outside the advisory's affected range, then pin that release's SHA. `--fix` does
not rewrite it. Ignoring a finding works through comments or config; see
[ignoring findings](../configuration.md#ignoring-findings).

**Options.** `rules.known-vulnerable-action.config.allow` lists advisory IDs to accept, with crosgha's or zizmor's
rule name ([config file](../configuration.md#config-file)).

```yaml
rules:
  known-vulnerable-action:
    config:
      allow:
        - GHSA-mrrh-fwg8-r2c3
```

All fixtures for this rule: [examples](../rule-examples.md#known-vulnerable-action).

## `outdated-action`

Referenced action versions should not lag behind the latest release.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | pedantic | online | none | none |

**What it reports.** It compares the version of each remote `uses` with the action repository's latest release and
reports the reference when it is older, comparing only as many numbers as the reference names: `@v4` is judged by its
major, `@v4.0.0` by all three, so a major-only ref that tracks its line is not reported. A repository without a
latest release, and a reference without a version, are skipped.

**Why it matters.** A reference that lags the latest release misses the fixes published since. GHAST's TP-1 and
CIAnalyser's `reportUpdateLag` check the same lag.

**Example.**

```yaml
name: Checkout
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v3
        with: { persist-credentials: false }
```

This rule queries the GitHub API, so it is skipped with `--offline`; the test fixture
[`outdated-action.before.yml`](https://github.com/reset-ufc/crosgha/blob/master/tests/fixtures/online/outdated-action.before.yml)
shows the two findings it reports, for `@v3` and for a SHA commented `# v4.0.0`.

Compliant:

```yaml
name: Checkout
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v4
```

**How to fix.** Move to the latest release, at the precision you already use: a major tag if you follow the
maintenance policy of that line, otherwise the full version and its SHA. `--fix` does not rewrite it. Ignoring a
finding works through comments or config; see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#outdated-action).

## `version-comment-mismatch`

The version comment next to a pinned SHA must name that SHA's tag.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | pedantic | online | none | zizmor: `ref-version-mismatch` |

**What it reports.** It reads SHA-pinned references only, and only when the comment on the `uses` line starts with a
version such as `v4.2.2`. It asks the action's repository for the commit that the named version resolves to and
reports when that version does not exist there, or exists but points to a different commit. A reference with no
version comment is skipped, because there is nothing to compare.

**Why it matters.** The comment is what a reviewer reads to decide whether the pin is current, so a comment that
names another commit makes the review approve code the workflow does not run. pinact's `--verify-comment` and
zizmor's `ref-version-mismatch` check the same mismatch.

**Example.**

```yaml
name: Checkout
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@1111111111111111111111111111111111111111 # v9.9.9
        with: { persist-credentials: false }
```

This rule queries the GitHub API, so it is skipped with `--offline`; the test fixture
[`version-comment-mismatch.before.yml`](https://github.com/reset-ufc/crosgha/blob/master/tests/fixtures/online/version-comment-mismatch.before.yml)
shows both messages, one for a version that points elsewhere and one for a version that does not exist.

Compliant:

```yaml
name: Checkout
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@1111111111111111111111111111111111111111 # v4.2.2
```

**How to fix.** Write the tag that points at the pinned SHA, or move the pin to the commit of the tag the comment
names. `--fix` does not rewrite it. Ignoring a finding works through comments or config; see
[ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#version-comment-mismatch).

## `impostor-commit`

A pinned SHA must belong to the named repository, not to a fork.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| error | regular | online | none | zizmor: `impostor-commit` |

**What it reports.** It reads SHA-pinned references only. It asks whether the SHA is reachable from the repository's
default branch, which the compare endpoint reports as `behind` or `identical`, and then whether any tag points at
it. A SHA that is neither is reported. It checks the default branch and the tags, not every branch, and a tag listing
that cannot be read produces no finding.

**Why it matters.** GitHub asks that, "When selecting a SHA, you should verify it is from the action's repository and
not a repository fork"
([actions-pin-commit-sha.md:1](https://github.com/github/docs/blob/44f009da482ef1470b37ffea456821014d892a1f/data/reusables/actions/actions-pin-commit-sha.md#L1)),
because a fork's commit runs the fork's code under the trusted repository's name. zizmor's `impostor-commit` audit
covers the same check.

**Example.**

```yaml
name: Checkout
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@2222222222222222222222222222222222222222 # v4.2.2
        with: { persist-credentials: false }
```

This rule queries the GitHub API, so it is skipped with `--offline`; the test fixture
[`impostor-commit.before.yml`](https://github.com/reset-ufc/crosgha/blob/master/tests/fixtures/online/impostor-commit.before.yml)
shows it reporting a diverged commit that no tag points at, and staying quiet for a commit that is a tag.

Compliant:

```yaml
name: Checkout
on: [push]
permissions: {}
jobs:
  build:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@1111111111111111111111111111111111111111 # v4.2.2
```

**How to fix.** Replace the SHA with one reachable from the named repository's default branch or one that a tag of
that repository points to. `--fix` does not rewrite it. Ignoring a finding works
through comments or config; see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#impostor-commit).

## `archived-action`

Actions from archived repositories receive no security fixes.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| warning | regular | online | none | zizmor: `archived-uses` |

**What it reports.** It asks the GitHub API whether the repository behind each remote `uses` is archived, and reports
every reference to one that is, including a reusable workflow called at job level. Without a token, or with
`--offline`, it reports the actions of a built-in list instead: `actions-rs/audit-check`, `actions-rs/cargo`,
`actions-rs/clippy`, `actions-rs/clippy-check`, `actions-rs/components-nightly`, `actions-rs/grcov`,
`actions-rs/install`, `actions-rs/tarpaulin`, `actions-rs/toolchain`, `actions/create-release`,
`actions/setup-elixir`, `actions/setup-haskell`, `actions/setup-ruby`, and `actions/upload-release-asset`.

**Why it matters.** An archived action receives no security fixes, so a vulnerability found in it stays in the
workflow that calls it. zizmor's `archived-uses` audit reports the same state.

**Example.**

```yaml
name: Release
on: [push]
permissions: {}
jobs:
  release:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: example/abandoned-release@v1
```

This rule queries the GitHub API, so it is skipped with `--offline` except for the built-in list above; the test
fixture
[`archived-action.before.yml`](https://github.com/reset-ufc/crosgha/blob/master/tests/fixtures/online/archived-action.before.yml)
shows the finding for an archived repository served by the fake API.

Compliant:

```yaml
name: Release
on: [push]
permissions: {}
jobs:
  release:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        with: { persist-credentials: false }
```

**How to fix.** Replace the archived action with a maintained one, or with a step that runs the tool itself, and
keep the same permissions the old step had. `--fix` does not rewrite it. Ignoring a finding works through comments
or config; see [ignoring findings](../configuration.md#ignoring-findings).

All fixtures for this rule: [examples](../rule-examples.md#archived-action).

## `missing-release-cooldown`

Dependabot updates should wait for a minimum release age.

| Severity | Persona | Scope | Fix | Equivalent in |
|---|---|---|---|---|
| note | pedantic | repository | none | zizmor: `dependabot-cooldown` |

**What it reports.** It reads `.github/dependabot.yml` or `.github/dependabot.yaml` and looks at each entry of
`updates`. An entry passes when `cooldown.default-days` is at least the minimum, 7 days by default; an entry with no
`cooldown` is judged by Dependabot's own 3-day default and therefore fails. Only `default-days` is read, so
`semver-major-days` and the other per-bump keys do not raise the value. Renovate configs are not read
([`src/rules/repository.rs`](https://github.com/reset-ufc/crosgha/blob/master/src/rules/repository.rs)).

**Why it matters.** `cooldown` lets updates be "delayed for a configurable number of days", and a new version "is
not considered for a version update until 3 days after its release"
([dependabot-options-reference.md:185](https://github.com/github/docs/blob/44f009da482ef1470b37ffea456821014d892a1f/content/code-security/reference/supply-chain-security/dependabot-options-reference.md#L185),
[dependabot-options-reference.md:194](https://github.com/github/docs/blob/44f009da482ef1470b37ffea456821014d892a1f/content/code-security/reference/supply-chain-security/dependabot-options-reference.md#L194)),
so the window is the time a malicious release has to be found before the update reaches the repository. The code
comment names zizmor's `dependabot-cooldown` audit for the same check.

**Example.** The fixture repository `tests/fixtures/repos/missing-release-cooldown/` holds one workflow,
`.github/workflows/ci.yml`, next to `.github/dependabot.yml` with a single `updates` entry and no `cooldown`:

```yaml
version: 2
updates:
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
```

From the repository root: `crosgha --offline --persona auditor tests/fixtures/repos/missing-release-cooldown`.

```text
note[missing-release-cooldown]: cooldown of 3 day(s) (Dependabot's default) is below the 7-day minimum
 --> tests/fixtures/repos/missing-release-cooldown/.github/dependabot.yml:3:5
  |
3 |   - package-ecosystem: github-actions
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ update `github-actions`
  |
  = help: Dependabot updates should wait for a minimum release age.
  = note: same as zizmor: dependabot-cooldown

Checked 1 workflow file: 10 findings (10 notes).
```

Compliant, from `tests/fixtures/repos/hardened/.github/dependabot.yml`:

```yaml
version: 2
updates:
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
    cooldown:
      default-days: 7
```

**How to fix.** Add `cooldown.default-days` with a value at or above the minimum to each `updates` entry, or lower
the minimum in the config when the repository wants faster updates. `--fix` does not rewrite it. Ignoring a finding
works through comments or config; see [ignoring findings](../configuration.md#ignoring-findings).

**Options.** `rules.missing-release-cooldown.config.days` sets the minimum in days; it is 7 when unset, and keys may
use zizmor's rule name ([config file](../configuration.md#config-file)).

```yaml
rules:
  missing-release-cooldown:
    config:
      days: 14
```

All fixtures for this rule: [examples](../rule-examples.md#missing-release-cooldown).
