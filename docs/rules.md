# Rules

Rule concepts cite their primary sources in [`src/rules/`](../src/rules/). `cargo run -- --list-rules` prints this
catalog from the code. **Scope** is what a rule reads: one workflow, the repository, or the GitHub API.
`retired-runtime` also checks the steps of each workflow, and `repo-org-settings` reads the repository's Actions
settings through the API. **Config** marks rules that read `rules.<id>.config`. **Examples** links to the rule's
non-compliant and compliant YAML, generated from its test fixtures.

| Rule | Taxonomy tag | Severity | Persona | Scope | Equivalent in | Config | Examples |
|---|---|---|---|---|---|---|---|
| `unpinned-package` | `unpinned-package` | warning | pedantic | workflow | zizmor: `adhoc-packages`, `unpinned-tools` |  | [examples](rule-examples.md#unpinned-package) |
| `unverified-remote-script` | `unverified-remote-script` | warning | regular | workflow |  |  | [examples](rule-examples.md#unverified-remote-script) |
| `typosquatted-action` | `typosquatting/impostor-commit` | error | regular | workflow | zizmor: `typosquat-uses` |  | [examples](rule-examples.md#typosquatted-action) |
| `script-injection` | `script-injection` | error | regular | workflow | zizmor: `template-injection`; actionlint: `expression` |  | [examples](rule-examples.md#script-injection) |
| `github-env-injection` | `github-env-injection` | error | regular | workflow | zizmor: `github-env` |  | [examples](rule-examples.md#github-env-injection) |
| `llm-prompt-injection` | `llm-prompt-injection` | error | regular | workflow |  |  | [examples](rule-examples.md#llm-prompt-injection) |
| `pwn-request` | `pwn-request` | error | regular | workflow |  |  | [examples](rule-examples.md#pwn-request) |
| `dangerous-trigger` | `dangerous-trigger` | warning | pedantic | workflow | zizmor: `dangerous-triggers` |  | [examples](rule-examples.md#dangerous-trigger) |
| `unsafe-if-condition` | `unsafe-if-condition` | warning | regular | workflow | zizmor: `bot-conditions`, `unsound-condition`; actionlint: `if-cond` |  | [examples](rule-examples.md#unsafe-if-condition) |
| `artifact-poisoning` | `artifact-poisoning` | warning | regular | workflow |  |  | [examples](rule-examples.md#artifact-poisoning) |
| `cache-poisoning` | `cache-poisoning` | warning | regular | workflow | zizmor: `cache-poisoning` |  | [examples](rule-examples.md#cache-poisoning) |
| `obfuscation` | `obfuscation` | warning | pedantic | workflow | zizmor: `obfuscation` |  | [examples](rule-examples.md#obfuscation) |
| `excessive-permissions` | `excessive-permissions` | warning | regular | workflow | zizmor: `excessive-permissions` |  | [examples](rule-examples.md#excessive-permissions) |
| `missing-permissions-block` | `missing-permissions-block` | warning | pedantic | workflow |  |  | [examples](rule-examples.md#missing-permissions-block) |
| `secrets-exposure` | `secrets-exposure` | warning | regular | workflow | zizmor: `overprovisioned-secrets` |  | [examples](rule-examples.md#secrets-exposure) |
| `secrets-inherit` | `secrets-inherit` | note | pedantic | workflow | zizmor: `secrets-inherit` |  | [examples](rule-examples.md#secrets-inherit) |
| `hardcoded-credentials` | `hardcoded-credentials` | error | regular | workflow | zizmor: `hardcoded-container-credentials`; actionlint: `credentials` |  | [examples](rule-examples.md#hardcoded-credentials) |
| `persist-credentials` | `persist-credentials` | warning | regular | workflow | zizmor: `artipacked` |  | [examples](rule-examples.md#persist-credentials) |
| `oidc-misconfig` | `oidc-misconfig` | warning | pedantic | workflow |  |  | [examples](rule-examples.md#oidc-misconfig) |
| `runtime-monitoring` | `runtime-monitoring` | note | auditor | workflow |  |  | [examples](rule-examples.md#runtime-monitoring) |
| `runner-versioning` | `runner-versioning` | note | pedantic | workflow |  |  | [examples](rule-examples.md#runner-versioning) |
| `missing-timeout` | `missing-timeout` | note | pedantic | workflow |  |  | [examples](rule-examples.md#missing-timeout) |
| `error-suppression` | `error-suppression` | note | pedantic | workflow |  |  | [examples](rule-examples.md#error-suppression) |
| `missing-concurrency` | `missing-concurrency` | note | pedantic | workflow | zizmor: `concurrency-limits` |  | [examples](rule-examples.md#missing-concurrency) |
| `broad-trigger` | `broad-trigger` | note | pedantic | workflow |  |  | [examples](rule-examples.md#broad-trigger) |
| `missing-fork-guard` | `missing-fork-guard` | note | pedantic | workflow |  |  | [examples](rule-examples.md#missing-fork-guard) |
| `missing-matrix` | `missing-matrix/test-diversity` | note | auditor | workflow |  |  | [examples](rule-examples.md#missing-matrix) |
| `cache-misuse` | `cache-misuse` | note | pedantic | workflow |  |  | [examples](rule-examples.md#cache-misuse) |
| `duplicated-steps` | `redundant/duplicated-steps` | note | pedantic | workflow |  |  | [examples](rule-examples.md#duplicated-steps) |
| `complex-job` | `long-job/complexity` | note | auditor | workflow |  |  | [examples](rule-examples.md#complex-job) |
| `readability` | `readability` | note | auditor | workflow | zizmor: `anonymous-definition` |  | [examples](rule-examples.md#readability) |
| `deprecated-command` | `deprecated-command/feature` | warning | regular | workflow | zizmor: `insecure-commands`; actionlint: `deprecated-commands` |  | [examples](rule-examples.md#deprecated-command) |
| `removed-feature` | `deprecated-command/feature` | error | regular | workflow | actionlint: `runner-label`, `action`, `deprecated-inputs` |  | [examples](rule-examples.md#removed-feature) |
| `schema-validity` | `syntax/schema-validity` | error | regular | workflow | actionlint: `syntax-check`, `expression` |  | [examples](rule-examples.md#schema-validity) |
| `embedded-script-lint` | `embedded-script-lint` | note | pedantic | workflow | actionlint: `shellcheck` |  | [examples](rule-examples.md#embedded-script-lint) |
| `insecure-protocol` | `insecure-protocol` | warning | regular | workflow | zizmor: `insecure-url-scheme` |  | [examples](rule-examples.md#insecure-protocol) |
| `unpinned-action` | `unpinned-action` | warning | pedantic | workflow | zizmor: `unpinned-uses`, `unpinned-images` | yes | [examples](rule-examples.md#unpinned-action) |
| `forbidden-action` | `untrusted-3rd-party-action` | warning | regular | workflow | zizmor: `forbidden-uses` | yes | [examples](rule-examples.md#forbidden-action) |
| `self-hosted-runner` | `self-hosted-runner` | warning | regular | workflow | zizmor: `self-hosted-runner` | yes | [examples](rule-examples.md#self-hosted-runner) |
| `known-vulnerable-action` | `known-vulnerable-action` | error | regular | online | zizmor: `known-vulnerable-actions` | yes | [examples](rule-examples.md#known-vulnerable-action) |
| `outdated-action` | `outdated-action` | note | pedantic | online |  |  | [examples](rule-examples.md#outdated-action) |
| `version-comment-mismatch` | `version-comment-mismatch` | warning | pedantic | online | zizmor: `ref-version-mismatch` |  | [examples](rule-examples.md#version-comment-mismatch) |
| `impostor-commit` | `typosquatting/impostor-commit` | error | regular | online | zizmor: `impostor-commit` |  | [examples](rule-examples.md#impostor-commit) |
| `archived-action` | `untrusted-3rd-party-action` | warning | regular | online | zizmor: `archived-uses` |  | [examples](rule-examples.md#archived-action) |
| `missing-release-cooldown` | `missing-release-cooldown` | note | pedantic | repository | zizmor: `dependabot-cooldown` | yes | [examples](rule-examples.md#missing-release-cooldown) |
| `missing-security-tooling` | `missing-security-tooling` | note | auditor | repository |  |  | [examples](rule-examples.md#missing-security-tooling) |
| `retired-runtime` | `deprecated-command/feature` | warning | regular | repository | actionlint: `action` |  | [examples](rule-examples.md#retired-runtime) |
| `repo-org-settings` | `repo-org-settings` | warning | pedantic | repository |  |  | [examples](rule-examples.md#repo-org-settings) |
