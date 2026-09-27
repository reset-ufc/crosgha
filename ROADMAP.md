# Roadmap

What is still planned. Shipped behavior is documented in [README.md](README.md) and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Shipped

- **CLI:** 48 rules covering the whole taxonomy (39 workflow, 4 repository, 5 online), verified fixes for
  `persist-credentials` and `removed-feature`, zizmor-compatible personas, ignores, and config,
  text/JSON/SARIF/annotation/agent output, `migrate --from zizmor|actionlint`, and `hook install|uninstall`.
- **Release:** release-please tags `vX.Y.Z` (`include-component-in-tag: false`); cargo-dist builds
  `crosgha-x86_64-unknown-linux-gnu.tar.xz` with a per-file `.sha256` and `sha256.sum` (`dist-workspace.toml`).
- **Action, report mode:** the root `action.yml` is a composite Action (no Docker or Node runtime). It takes a superset
  of `zizmorcore/zizmor-action`'s inputs, installs the release named by its own `Cargo.toml` after checking the
  `.sha256`, uploads SARIF with a SHA-pinned `upload-sarif`, writes the job summary, sets the `output-file` and
  `findings` outputs, and fails fast on non-Linux runners. There is no moving `v1` tag: mutable refs are one of the
  smells we detect.

## Next

1. **PR mode** (`fix: pull-request`) and opt-in auto-merge, as designed [below](#pr-and-auto-merge-modes). Inputs and
   guards exist; the branch, commit, and PR operations are pending.
2. **More verified fixes.** Candidates whose change needs no project context: `deprecated-command` (environment
   files), `unsafe-if-condition` (unwrap `${{ }}`), `embedded-script-lint` (quote the path).
3. **More `migrate --from` sources** (poutine, ghalint, octoscan) once their Action steps are catalogued.
4. **Dogfood the Action** (`uses: ./`) in our own CI.
5. **Publishing:** enable GitHub immutable releases (currently off), and list the Action on the Marketplace once the
   repo is public (the root `action.yml` already has `name`, `description`, and `branding`).

Later, only on demand: an `aarch64-unknown-linux-gnu` or musl build when an ARM runner user needs it.

## PR and auto-merge modes

Manual by design: automation is an explicit, at-your-own-risk opt-in. The modes were decided on 2026-09-25.

| Mode | Inputs | What happens | Token |
|---|---|---|---|
| Report (default) | `fix: none` | Findings as SARIF, job summary, and `findings` output; nothing is changed | default `GITHUB_TOKEN` (`contents: read`, + `security-events: write` for SARIF upload) |
| Pull request | `fix: pull-request`, `fix-token` | Runs `crosgha --fix`, commits to `crosgha/fix-<run_id>`, and opens a PR listing each fix and its rule. **A human reviews and merges.** | GitHub App or fine-grained PAT with Contents, Pull requests, and Workflows: write (see below) |
| Auto-merge (opt-in, at user's risk) | `fix: pull-request`, `fix-token`, `dangerously-auto-merge: true` | Same PR, then enables GitHub auto-merge, so it merges itself once all required reviews and status checks pass | same as Pull request |

Auto-merge behavior (verified against the [GitHub auto-merge docs](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/automatically-merging-a-pull-request), 2026-09-25):
- Auto-merge "must be enabled for the repository".
- It "merges a pull request automatically after all required reviews and status checks pass".
- The option is "shown only on pull requests that cannot be merged immediately". In other words, it depends on branch protection or rulesets.
- Enabling it requires write permission.

Design choices:
- The flag is named `dangerously-*`, like `--dangerously-skip-permissions`, so the risk is visible in the caller's YAML.
- The job summary repeats a warning on every run that uses the flag.
- The action **never merges directly**. If the base branch has no required status checks, it fails fast instead of merging an unchecked workflow change.

### Why PR mode cannot use `GITHUB_TOKEN` (verified 2026-09-25)

Every fix crosgha makes touches `.github/workflows/`, and the default token cannot write there:

1. `GITHUB_TOKEN` has no `workflows` permission. The scopes it can receive are `actions`, `artifact-metadata`, `attestations`, `checks`, `code-quality`, `contents`, `deployments`, `discussions`, `id-token`, `issues`, `packages`, `pages`, `pull-requests`, `security-events`, `statuses`, and `vulnerability-alerts`. Source: [workflow syntax, `permissions`](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax).
2. Writing to `.github/workflows/` needs that extra permission. The REST docs say: "The workflow scope is also required in order to modify files in the .github/workflows directory." ([repos/contents](https://docs.github.com/en/rest/repos/contents)). For GitHub Apps and fine-grained PATs, the equivalent is the repository permission **Workflows: write**, which gates `PUT /contents/{path}` and `POST/PATCH /git/refs` ([GitHub App permissions](https://docs.github.com/en/rest/authentication/permissions-required-for-github-apps), [fine-grained PAT permissions](https://docs.github.com/en/rest/authentication/permissions-required-for-fine-grained-personal-access-tokens)).
3. GitHub's own guidance for permissions missing from `GITHUB_TOKEN` is to create a GitHub App and generate an installation access token inside the workflow, or to use a personal access token ([authenticate with GITHUB_TOKEN](https://docs.github.com/en/actions/tutorials/authenticate-with-github_token)).
4. Side benefit of a non-default token: a PR opened with `GITHUB_TOKEN` gets `pull_request` runs in an **approval-required** state, while a GitHub App installation token or PAT runs CI without approval ([GITHUB_TOKEN concepts](https://docs.github.com/en/actions/concepts/security/github_token)).

### Consequences for the PR-mode design
- **Recommended token:** a GitHub App installation token minted in the caller's job, with `actions/create-github-app-token` pinned by SHA. App permissions: Contents: write, Pull requests: write, Workflows: write.
- **Fallback token:** a fine-grained PAT with the same three permissions.
- The repo setting "Allow GitHub Actions to create and approve pull requests" applies only to `GITHUB_TOKEN`, so PR mode does not need it.
- The action fails fast, with a message linking to this section, when `fix: pull-request` is set without `fix-token`, or when `dangerously-auto-merge` is set without `fix: pull-request`. `token` stays the online-rules token (default `github.token`), as in zizmor-action.
