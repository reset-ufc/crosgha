# Rules

Rule concepts cite their primary sources in [`src/rules/`](../src/rules/). `cargo run -- --list-rules` prints this
catalog from the code. **Scope** is what a rule reads: one file (`workflow`), the repository, or the GitHub API.
`repo-org-settings` reads the repository's Actions settings through the API. **Actions** is what a rule checks in
action metadata files ([which ones are scanned](usage.md#actions)): `steps` for a composite action's `runs.steps`, its
`uses` lines, and text anywhere in the file; `metadata` for `runs` and the required keys. An empty cell means the rule
reads keys only workflows have, such as triggers, jobs, `permissions`, or `defaults`. **Config** marks rules that read
`rules.<id>.config`. **Examples** links to the rule's non-compliant and compliant YAML, generated from its test
fixtures.

| Rule | Taxonomy tag | Severity | Persona | Scope | Actions | Equivalent in | Config | Examples |
|---|---|---|---|---|---|---|---|---|
| `unpinned-package` | `unpinned-package` | warning | pedantic | workflow | steps | zizmor: `adhoc-packages`, `unpinned-tools` |  | [examples](rule-examples.md#unpinned-package) |
| `unverified-remote-script` | `unverified-remote-script` | warning | regular | workflow | steps |  |  | [examples](rule-examples.md#unverified-remote-script) |
| `typosquatted-action` | `typosquatting/impostor-commit` | error | regular | workflow | steps | zizmor: `typosquat-uses` |  | [examples](rule-examples.md#typosquatted-action) |
| `script-injection` | `script-injection` | error | regular | workflow | steps | zizmor: `template-injection`; actionlint: `expression`; ghlint: `ShellScriptInjection`, `JSScriptInjection` |  | [examples](rule-examples.md#script-injection) |
| `github-env-injection` | `github-env-injection` | error | regular | workflow | steps | zizmor: `github-env` |  | [examples](rule-examples.md#github-env-injection) |
| `llm-prompt-injection` | `llm-prompt-injection` | error | regular | workflow | steps |  |  | [examples](rule-examples.md#llm-prompt-injection) |
| `pwn-request` | `pwn-request` | error | regular | workflow |  |  |  | [examples](rule-examples.md#pwn-request) |
| `dangerous-trigger` | `dangerous-trigger` | warning | pedantic | workflow |  | zizmor: `dangerous-triggers` |  | [examples](rule-examples.md#dangerous-trigger) |
| `unsafe-if-condition` | `unsafe-if-condition` | warning | regular | workflow | steps | zizmor: `bot-conditions`, `unsound-condition`; actionlint: `if-cond`; ghlint: `DoubleCurlyIf` |  | [examples](rule-examples.md#unsafe-if-condition) |
| `artifact-poisoning` | `artifact-poisoning` | warning | regular | workflow |  |  |  | [examples](rule-examples.md#artifact-poisoning) |
| `cache-poisoning` | `cache-poisoning` | warning | regular | workflow |  | zizmor: `cache-poisoning` |  | [examples](rule-examples.md#cache-poisoning) |
| `obfuscation` | `obfuscation` | warning | pedantic | workflow | steps | zizmor: `obfuscation` |  | [examples](rule-examples.md#obfuscation) |
| `excessive-permissions` | `excessive-permissions` | warning | regular | workflow |  | zizmor: `excessive-permissions` |  | [examples](rule-examples.md#excessive-permissions) |
| `workflow-level-permissions` | `excessive-permissions` | note | auditor | workflow |  | ghlint: `ExplicitJobPermissions` |  | [examples](rule-examples.md#workflow-level-permissions) |
| `missing-permissions-block` | `missing-permissions-block` | warning | pedantic | workflow |  | ghlint: `MissingJobPermissions` |  | [examples](rule-examples.md#missing-permissions-block) |
| `secrets-exposure` | `secrets-exposure` | warning | regular | workflow |  | zizmor: `overprovisioned-secrets` |  | [examples](rule-examples.md#secrets-exposure) |
| `secrets-inherit` | `secrets-inherit` | note | pedantic | workflow |  | zizmor: `secrets-inherit` |  | [examples](rule-examples.md#secrets-inherit) |
| `hardcoded-credentials` | `hardcoded-credentials` | error | regular | workflow | steps | zizmor: `hardcoded-container-credentials`; actionlint: `credentials` |  | [examples](rule-examples.md#hardcoded-credentials) |
| `persist-credentials` | `persist-credentials` | warning | regular | workflow | steps | zizmor: `artipacked` |  | [examples](rule-examples.md#persist-credentials) |
| `oidc-misconfig` | `oidc-misconfig` | warning | pedantic | workflow |  |  |  | [examples](rule-examples.md#oidc-misconfig) |
| `runtime-monitoring` | `runtime-monitoring` | note | auditor | workflow |  |  |  | [examples](rule-examples.md#runtime-monitoring) |
| `runner-versioning` | `runner-versioning` | note | pedantic | workflow |  |  |  | [examples](rule-examples.md#runner-versioning) |
| `missing-timeout` | `missing-timeout` | note | pedantic | workflow |  | ghlint: `MissingJobTimeout` |  | [examples](rule-examples.md#missing-timeout) |
| `error-suppression` | `error-suppression` | note | pedantic | workflow | steps | ghlint: `FailFastUploadArtifact`, `FailFastSoftpropsGhRelease`, `FailFastPublishUnitTestResults`, `FailFastPeterEvansCreatePullRequest` |  | [examples](rule-examples.md#error-suppression) |
| `missing-concurrency` | `missing-concurrency` | note | pedantic | workflow |  | zizmor: `concurrency-limits` |  | [examples](rule-examples.md#missing-concurrency) |
| `broad-trigger` | `broad-trigger` | note | pedantic | workflow |  |  |  | [examples](rule-examples.md#broad-trigger) |
| `missing-fork-guard` | `missing-fork-guard` | note | pedantic | workflow |  |  |  | [examples](rule-examples.md#missing-fork-guard) |
| `missing-matrix` | `missing-matrix/test-diversity` | note | auditor | workflow |  |  |  | [examples](rule-examples.md#missing-matrix) |
| `cache-misuse` | `cache-misuse` | note | pedantic | workflow | steps |  |  | [examples](rule-examples.md#cache-misuse) |
| `duplicated-steps` | `redundant/duplicated-steps` | note | pedantic | workflow | steps |  |  | [examples](rule-examples.md#duplicated-steps) |
| `redundant-setting` | `redundant/duplicated-steps` | note | auditor | workflow | steps | ghlint: `EmptyWorkflowEnv`, `EmptyJobEnv`, `EmptyStepEnv`, `RedundantShell`, `RedundantDefaultShell`, `DuplicateShellOnSteps` |  | [examples](rule-examples.md#redundant-setting) |
| `complex-job` | `long-job/complexity` | note | auditor | workflow | steps | ghlint: `TooManySteps`, `TooManyJobs` |  | [examples](rule-examples.md#complex-job) |
| `readability` | `readability` | note | auditor | workflow | steps | zizmor: `anonymous-definition`; ghlint: `MissingWorkflowName`, `MissingJobName`, `MissingStepName`, `WorkflowIdNaming`, `JobIdNaming`, `StepIdNaming`, `SimilarStepId`, `MissingShell`, `PreferGitHubToken`, `DoubleCurlyIf`, `NeverUseAlways`, `NegativeStatusCheck` |  | [examples](rule-examples.md#readability) |
| `deprecated-command` | `deprecated-command/feature` | warning | regular | workflow | steps | zizmor: `insecure-commands`; actionlint: `deprecated-commands` |  | [examples](rule-examples.md#deprecated-command) |
| `removed-feature` | `deprecated-command/feature` | error | regular | workflow | steps | actionlint: `runner-label`, `action`, `deprecated-inputs` |  | [examples](rule-examples.md#removed-feature) |
| `schema-validity` | `syntax/schema-validity` | error | regular | workflow | metadata | actionlint: `syntax-check`, `expression`; ghlint: `JsonSchemaValidation` |  | [examples](rule-examples.md#schema-validity) |
| `embedded-script-lint` | `embedded-script-lint` | note | pedantic | workflow | steps | actionlint: `shellcheck`; ghlint: `SafeEnvironmentFileRedirect`, `EnvironmentFileOverwritten` |  | [examples](rule-examples.md#embedded-script-lint) |
| `insecure-protocol` | `insecure-protocol` | warning | regular | workflow | steps | zizmor: `insecure-url-scheme` |  | [examples](rule-examples.md#insecure-protocol) |
| `gh-cli-environment` | `embedded-script-lint` | warning | regular | workflow |  | ghlint: `MissingGhToken`, `MissingGhHost`, `MissingGhRepo` |  | [examples](rule-examples.md#gh-cli-environment) |
| `retired-runtime` | `deprecated-command/feature` | warning | regular | workflow | steps, metadata | actionlint: `action` |  | [examples](rule-examples.md#retired-runtime) |
| `unpinned-action` | `unpinned-action` | warning | pedantic | workflow | steps, metadata | zizmor: `unpinned-uses`, `unpinned-images` | yes | [examples](rule-examples.md#unpinned-action) |
| `forbidden-action` | `untrusted-3rd-party-action` | warning | regular | workflow | steps | zizmor: `forbidden-uses` | yes | [examples](rule-examples.md#forbidden-action) |
| `self-hosted-runner` | `self-hosted-runner` | warning | regular | workflow |  | zizmor: `self-hosted-runner` | yes | [examples](rule-examples.md#self-hosted-runner) |
| `known-vulnerable-action` | `known-vulnerable-action` | error | regular | online | steps | zizmor: `known-vulnerable-actions` | yes | [examples](rule-examples.md#known-vulnerable-action) |
| `outdated-action` | `outdated-action` | note | pedantic | online | steps |  |  | [examples](rule-examples.md#outdated-action) |
| `version-comment-mismatch` | `version-comment-mismatch` | warning | pedantic | online | steps | zizmor: `ref-version-mismatch` |  | [examples](rule-examples.md#version-comment-mismatch) |
| `impostor-commit` | `typosquatting/impostor-commit` | error | regular | online | steps | zizmor: `impostor-commit` |  | [examples](rule-examples.md#impostor-commit) |
| `archived-action` | `untrusted-3rd-party-action` | warning | regular | online | steps | zizmor: `archived-uses` |  | [examples](rule-examples.md#archived-action) |
| `missing-release-cooldown` | `missing-release-cooldown` | note | pedantic | repository |  | zizmor: `dependabot-cooldown` | yes | [examples](rule-examples.md#missing-release-cooldown) |
| `missing-security-tooling` | `missing-security-tooling` | note | auditor | repository |  |  |  | [examples](rule-examples.md#missing-security-tooling) |
| `repo-org-settings` | `repo-org-settings` | warning | pedantic | repository |  |  |  | [examples](rule-examples.md#repo-org-settings) |
