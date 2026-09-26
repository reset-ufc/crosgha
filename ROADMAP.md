# Roadmap

The GitHub Action and SARIF integration below are planned. `action.yml` now
records the Action inputs and outputs, and its entry point validates runner and mode inputs
before failing explicitly; it does not install or run the binary yet. The CLI
pipeline is organized in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md); it scans
workflows and implements the `persist-credentials` finding and verified fix.

## Distribution

- **Now:** the release binary is built for `x86_64-unknown-linux-gnu` only (cargo-dist, `dist-workspace.toml`).
  The release ships `crosgha-x86_64-unknown-linux-gnu.tar.xz`, a per-file `.sha256`, and `sha256.sum`.
- **Planned:** a GitHub Action that runs the released binary, so a repository can use `uses: <owner>/crosgha@<sha>`.

## Planned: GitHub Action packaging

Goal: run crosgha on a repository's `.github/workflows/` inside CI and report the findings in the Security tab.

1. **Action type: composite.** The root `action.yml` declares a composite Action, with no Docker or JavaScript runtime.
   - Planned: download the release asset for the requested version.
   - Planned: verify the asset against the release's `.sha256` before running.
   - A composite action avoids the Node runtime deprecations that JavaScript actions go through.
2. **Inputs:**
   - `version`: defaults to the version the action ref was released with, never "latest".
   - `path`: defaults to `.github/workflows`.
   - `format`: `sarif` | `json` | `text`.
   - `fail-on`: severity threshold.
3. **Outputs:**
   - SARIF file path (the `sarif-file` output).
   - Findings count.
   - Optional upload through `github/codeql-action/upload-sarif`, pinned by SHA, with `security-events: write` requested only by the caller's job.
4. **Runners:** Linux only, matching the binary. On macOS and Windows runners the action fails fast with a clear message. An `aarch64-unknown-linux-gnu` or musl build is added only when an ARM runner user needs it.
5. **Dogfood:** our own CI uses the action at the current commit (`uses: ./`) on our own workflows.
6. **Versioning:**
   - Action releases are the same SemVer tags cut by release-please (`vX.Y.Z`).
   - No moving major tag (`v1`): mutable refs are one of the smells we detect. Docs tell users to pin the full SHA with a `# vX.Y.Z` comment.
   - Enable GitHub immutable releases if available on the repo.
7. **Marketplace:** requires a public repo, `action.yml` at the root, and `name`/`description`/`branding`.

Decisions (2026-09-25):
- **Same repo.** `action.yml` lives at the root of this repo, so the action and the binary share one version and one release.
- **Manual by design; automation is an explicit, at-your-own-risk opt-in.**

  | Mode | Inputs | What happens | Token |
  |---|---|---|---|
  | Report (default) | `fix: none` | Findings as SARIF, job summary, and `findings` output; nothing is changed | default `GITHUB_TOKEN` (`contents: read`, + `security-events: write` for SARIF upload) |
  | Pull request | `fix: pull-request`, `token` | Runs `crosgha --fix`, commits to `crosgha/fix-<run_id>`, and opens a PR listing each fix and its rule. **A human reviews and merges.** | GitHub App or fine-grained PAT with Contents, Pull requests, and Workflows: write (see below) |
  | Auto-merge (opt-in, at user's risk) | `fix: pull-request`, `token`, `dangerously-auto-merge: true` | Same PR, then enables GitHub auto-merge, so it merges itself once all required reviews and status checks pass | same as Pull request |

  Auto-merge rules (verified against the [GitHub auto-merge docs](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/automatically-merging-a-pull-request), 2026-09-25):
  - Auto-merge "must be enabled for the repository".
  - It "merges a pull request automatically after all required reviews and status checks pass".
  - The option is "shown only on pull requests that cannot be merged immediately". In other words, it depends on branch protection or rulesets.
  - Enabling it requires write permission.

  Our design choices:
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
- The action fails fast, with a message linking to this section, when `fix: pull-request` is set without `token`, or when `dangerously-auto-merge` is set without `fix: pull-request`.

## Tool

Detection and deterministic fixing of the curated smell taxonomy. The taxonomy and rule concepts are in the research workspace (`CLAUDE.workspace.md` → `../smell-matrix.md`).
Rules enter the shared detection and fix path only when their semantics,
primary-source citation, and before/after fixture are ready. JSON/SARIF
renderers and verified release installation are the next prerequisites for the
report-only Action mode. PR and auto-merge modes follow the verified report
and fix paths.
