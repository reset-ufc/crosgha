# Rules

Rule concepts cite their primary sources in [`src/rules/`](../src/rules/). `cargo run -- --list-rules` prints this
catalog from the code. **Scope** is what a rule reads: one workflow, the repository, or the GitHub API.
`retired-runtime` also checks the steps of each workflow, and `repo-org-settings` reads the repository's Actions
settings through the API. **Config** marks rules that read `rules.<id>.config`.

| Rule | Taxonomy tag | Severity | Persona | Scope | Equivalent in | Config |
|---|---|---|---|---|---|---|
| `unpinned-package` | `unpinned-package` | warning | pedantic | workflow | zizmor: `adhoc-packages`, `unpinned-tools` |  |
| `unverified-remote-script` | `unverified-remote-script` | warning | regular | workflow |  |  |
| `typosquatted-action` | `typosquatting/impostor-commit` | error | regular | workflow | zizmor: `typosquat-uses` |  |
| `script-injection` | `script-injection` | error | regular | workflow | zizmor: `template-injection`; actionlint: `expression` |  |
| `github-env-injection` | `github-env-injection` | error | regular | workflow | zizmor: `github-env` |  |
| `llm-prompt-injection` | `llm-prompt-injection` | error | regular | workflow |  |  |
| `pwn-request` | `pwn-request` | error | regular | workflow |  |  |
| `dangerous-trigger` | `dangerous-trigger` | warning | pedantic | workflow | zizmor: `dangerous-triggers` |  |
| `unsafe-if-condition` | `unsafe-if-condition` | warning | regular | workflow | zizmor: `bot-conditions`, `unsound-condition`; actionlint: `if-cond` |  |
| `artifact-poisoning` | `artifact-poisoning` | warning | regular | workflow |  |  |
| `cache-poisoning` | `cache-poisoning` | warning | regular | workflow | zizmor: `cache-poisoning` |  |
| `obfuscation` | `obfuscation` | warning | pedantic | workflow | zizmor: `obfuscation` |  |
| `excessive-permissions` | `excessive-permissions` | warning | regular | workflow | zizmor: `excessive-permissions` |  |
| `missing-permissions-block` | `missing-permissions-block` | warning | pedantic | workflow |  |  |
| `secrets-exposure` | `secrets-exposure` | warning | regular | workflow | zizmor: `overprovisioned-secrets` |  |
| `secrets-inherit` | `secrets-inherit` | note | pedantic | workflow | zizmor: `secrets-inherit` |  |
| `hardcoded-credentials` | `hardcoded-credentials` | error | regular | workflow | zizmor: `hardcoded-container-credentials`; actionlint: `credentials` |  |
| `persist-credentials` | `persist-credentials` | warning | regular | workflow | zizmor: `artipacked` |  |
| `oidc-misconfig` | `oidc-misconfig` | warning | pedantic | workflow |  |  |
| `runtime-monitoring` | `runtime-monitoring` | note | auditor | workflow |  |  |
| `runner-versioning` | `runner-versioning` | note | pedantic | workflow |  |  |
| `missing-timeout` | `missing-timeout` | note | pedantic | workflow |  |  |
| `error-suppression` | `error-suppression` | note | pedantic | workflow |  |  |
| `missing-concurrency` | `missing-concurrency` | note | pedantic | workflow | zizmor: `concurrency-limits` |  |
| `broad-trigger` | `broad-trigger` | note | pedantic | workflow |  |  |
| `missing-fork-guard` | `missing-fork-guard` | note | pedantic | workflow |  |  |
| `missing-matrix` | `missing-matrix/test-diversity` | note | auditor | workflow |  |  |
| `cache-misuse` | `cache-misuse` | note | pedantic | workflow |  |  |
| `duplicated-steps` | `redundant/duplicated-steps` | note | pedantic | workflow |  |  |
| `complex-job` | `long-job/complexity` | note | auditor | workflow |  |  |
| `readability` | `readability` | note | auditor | workflow | zizmor: `anonymous-definition` |  |
| `deprecated-command` | `deprecated-command/feature` | warning | regular | workflow | zizmor: `insecure-commands`; actionlint: `deprecated-commands` |  |
| `removed-feature` | `deprecated-command/feature` | error | regular | workflow | actionlint: `runner-label`, `action`, `deprecated-inputs` |  |
| `schema-validity` | `syntax/schema-validity` | error | regular | workflow | actionlint: `syntax-check`, `expression` |  |
| `embedded-script-lint` | `embedded-script-lint` | note | pedantic | workflow | actionlint: `shellcheck` |  |
| `insecure-protocol` | `insecure-protocol` | warning | regular | workflow | zizmor: `insecure-url-scheme` |  |
| `unpinned-action` | `unpinned-action` | warning | pedantic | workflow | zizmor: `unpinned-uses`, `unpinned-images` | yes |
| `forbidden-action` | `untrusted-3rd-party-action` | warning | regular | workflow | zizmor: `forbidden-uses` | yes |
| `self-hosted-runner` | `self-hosted-runner` | warning | regular | workflow | zizmor: `self-hosted-runner` | yes |
| `known-vulnerable-action` | `known-vulnerable-action` | error | regular | online | zizmor: `known-vulnerable-actions` | yes |
| `outdated-action` | `outdated-action` | note | pedantic | online |  |  |
| `version-comment-mismatch` | `version-comment-mismatch` | warning | pedantic | online | zizmor: `ref-version-mismatch` |  |
| `impostor-commit` | `typosquatting/impostor-commit` | error | regular | online | zizmor: `impostor-commit` |  |
| `archived-action` | `untrusted-3rd-party-action` | warning | regular | online | zizmor: `archived-uses` |  |
| `missing-release-cooldown` | `missing-release-cooldown` | note | pedantic | repository | zizmor: `dependabot-cooldown` | yes |
| `missing-security-tooling` | `missing-security-tooling` | note | auditor | repository |  |  |
| `retired-runtime` | `deprecated-command/feature` | warning | regular | repository | actionlint: `action` |  |
| `repo-org-settings` | `repo-org-settings` | warning | pedantic | repository |  |  |
