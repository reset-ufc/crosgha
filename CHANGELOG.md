# Changelog

## [0.11.2](https://github.com/meiazero/crosgha/compare/v0.11.1...v0.11.2) (2026-10-03)


### Fixed

* **action:** fail on non-X64 runners and on any unexpected exit status ([5db1996](https://github.com/meiazero/crosgha/commit/5db1996c1556507884b24643a18e773c67703f06))
* **action:** scan every path of a multi-line inputs value ([d7b3b0b](https://github.com/meiazero/crosgha/commit/d7b3b0b291e6d1b75759212aa3410e9318004b4b))
* **action:** stop workflow commands in relayed scan output ([401ad50](https://github.com/meiazero/crosgha/commit/401ad50f128c1043f2a31161b1d0c3f6a69f38eb))
* allow plain HTTP only for a bare loopback release origin ([4f92be9](https://github.com/meiazero/crosgha/commit/4f92be961cf7dc510ad744ad609ed2d267cb6382))
* audit fixes across the fix pipeline, rules, config, API, Action, and performance ([a089c76](https://github.com/meiazero/crosgha/commit/a089c76003e791a119e259553f92f7b962beedb5))
* **cli:** apply the config each input discovers ([94e3ed8](https://github.com/meiazero/crosgha/commit/94e3ed8a535244b65496c771956499b8758ab7e7))
* **cli:** let --no-config override CROSGHA_CONFIG ([3c5bbba](https://github.com/meiazero/crosgha/commit/3c5bbba94744927f567a531e59ce6eebdb758011))
* **cli:** scan files reached through overlapping inputs once ([020e831](https://github.com/meiazero/crosgha/commit/020e83111ffacaf4bb48863075092eb3e6f2c735))
* **config:** reject ignore entries with a malformed line ([58e4f01](https://github.com/meiazero/crosgha/commit/58e4f014aafc41e6b4cc61a068caf0282058bdef))
* **config:** skip the workflows directory when a single workflow is scanned ([caf1120](https://github.com/meiazero/crosgha/commit/caf1120309003a03fcfd806e06aedda3a8e7c10f))
* **fix:** apply fixes next to comments, in CRLF files, and without a final newline ([54af6b4](https://github.com/meiazero/crosgha/commit/54af6b4591e5dcca5d532af987dadf630a75364a))
* **fix:** keep the finding when its fix is rejected ([ac8a4a4](https://github.com/meiazero/crosgha/commit/ac8a4a46bfdc780c7f0b96b5f49347a01687b545))
* **fix:** refuse a symlinked workflow only when a fix must be written to it ([9eaeb57](https://github.com/meiazero/crosgha/commit/9eaeb57072fa01c5ee0fd449e022cfcf6852f08b))
* **github:** keep offline findings when a GitHub API request fails ([d05aa93](https://github.com/meiazero/crosgha/commit/d05aa9353e8849f4bf3a81b539237a774b4684d2))
* **install:** run the new binary before replacing the installed one ([2db2286](https://github.com/meiazero/crosgha/commit/2db22860c037150c7c172176a1abfb44d5a7978e))
* **migrate:** keep commands that share a step with the zizmor install ([981a2cb](https://github.com/meiazero/crosgha/commit/981a2cbc7d3a6efc83ce38fcee21ee6edf3d7817))
* **migrate:** note output formats the crosgha Action cannot produce ([9023de8](https://github.com/meiazero/crosgha/commit/9023de86d4d3c9f40fec6631683a1d0ad373e539))
* **online:** check every tag page before reporting an impostor commit ([779510b](https://github.com/meiazero/crosgha/commit/779510b940b20c47015c5fa6161de8791d3514a5))
* **online:** look up advisories for the release a major-only ref points to ([854746f](https://github.com/meiazero/crosgha/commit/854746fd55fefe2f7a44eff4dfb5824e2abcf69e))
* **online:** skip repo-org-settings when the token cannot read Actions settings ([c216cb1](https://github.com/meiazero/crosgha/commit/c216cb1455f88a6fc623886592a8668349fb87e0))
* **report:** emit UTF-16 SARIF columns and percent-encoded URIs ([9a0fbbd](https://github.com/meiazero/crosgha/commit/9a0fbbdef2e1a84f7de8df8fab090dfdaceb71a9))
* **report:** report only the fixes that were written ([2bd7e37](https://github.com/meiazero/crosgha/commit/2bd7e37cf4422890f67be03c49bff38fb31f017d))
* **rules:** align actionlint equivalents with the taxonomy ([9fcaaec](https://github.com/meiazero/crosgha/commit/9fcaaec879187e2dfc68d747c4e062aebf13bf4a))
* **rules:** count only cache saves after untrusted checkout ([082ff19](https://github.com/meiazero/crosgha/commit/082ff194dd78a1b00bd858490c40d32dde45dd69))
* **rules:** detect fork and merge-commit checkouts in pwn-request ([c90bd4d](https://github.com/meiazero/crosgha/commit/c90bd4d3887537ec863eafd49e53993fc9ff4ccc))
* **rules:** drop the stray article in missing-security-tooling messages ([9087012](https://github.com/meiazero/crosgha/commit/9087012adcb70020a4156bda7460cb08f3afa64f))
* **rules:** expand matrix runner values that are label lists ([532bf58](https://github.com/meiazero/crosgha/commit/532bf5893683713d44dbb72ec872366d2789956c))
* **rules:** find environment file writes through tee, PowerShell, groups, and heredocs ([affca52](https://github.com/meiazero/crosgha/commit/affca524e24101683d9d0ef4f8ff393bd9db3d78))
* **rules:** ignore remote downloads piped as data ([ca65ae3](https://github.com/meiazero/crosgha/commit/ca65ae3c870195fc5d0fa33d2147e5ef25aae997))
* **rules:** keep steps whose outputs are read out of duplicated-steps ([0a197f5](https://github.com/meiazero/crosgha/commit/0a197f500a08aae572acc6bb35ee9ba364319fc9))
* **rules:** match actions/checkout case-insensitively in persist-credentials ([b56d83d](https://github.com/meiazero/crosgha/commit/b56d83d6c92345d712dccedeaafc9114f9d50c43))
* **rules:** match untrusted contexts in index syntax, any case, and toJSON objects ([dfbf8b2](https://github.com/meiazero/crosgha/commit/dfbf8b238bab08c827842658ee34aa045819d123))
* **rules:** recognize custom bash and sh shell paths ([f7f760a](https://github.com/meiazero/crosgha/commit/f7f760a635b8f9b65116384ea99c37802a7833b6))
* **rules:** report older retired hosted runner images ([26934eb](https://github.com/meiazero/crosgha/commit/26934eb7d08cec766900e69c736e3acc7805cb2d))
* **rules:** report only bot-name checks that grant a spoofed actor ([4b6d4be](https://github.com/meiazero/crosgha/commit/4b6d4be7d521284bb7bc4057f9d6a6623247f546))
* **rules:** report opt-out variables only when set to true ([a5fe15d](https://github.com/meiazero/crosgha/commit/a5fe15dd4e95e862b54e7eefb98dfd031254b7dc))
* **rules:** require an exact npm package version ([bf481d7](https://github.com/meiazero/crosgha/commit/bf481d729649773086c09b2a368154680e970d9e))
* **rules:** say exact version in unpinned-package messages ([79798bb](https://github.com/meiazero/crosgha/commit/79798bbd26b71a35bee10e0f9cecb16746436072))
* **rules:** stop counting the shell test builtin as a test suite ([4863fb2](https://github.com/meiazero/crosgha/commit/4863fb2848cd5363e436b563e81cf448a862d538))
* **rules:** treat jobs that need a fork-guarded job as guarded ([fd0ee99](https://github.com/meiazero/crosgha/commit/fd0ee9967ff54997e7e1e95d3ce3183cfe13bcef))
* **rules:** validate permission access levels ([aafdc6d](https://github.com/meiazero/crosgha/commit/aafdc6d42c08499c92963da201c84686b588beab))
* **scan:** read workflows only directly inside .github/workflows ([25f7aee](https://github.com/meiazero/crosgha/commit/25f7aee81ab950a928e26f5993c52bfbdcb721f7))


### Changed

* **fix:** reuse each validation's findings as the next baseline ([a2ae88f](https://github.com/meiazero/crosgha/commit/a2ae88fcde68fc6b655e8b1f65c84d4333646d08))
* **rules:** look up each version comment once per workflow ([b3cb4ae](https://github.com/meiazero/crosgha/commit/b3cb4ae9dbd768e75815289d27a09e54295ff410))
* **rules:** reuse parsed workflows in repository rules ([9e07bc3](https://github.com/meiazero/crosgha/commit/9e07bc32cc380d1d5b5b7f79a8c367fe11d99286))
* **rules:** skip the ignore lookup in files without an ignore comment ([9c6887e](https://github.com/meiazero/crosgha/commit/9c6887e48051a596ac30ca30b725f2f9f39fb46d))


### Documentation

* describe rejected and unwritten fixes in the report ([9fdc4d2](https://github.com/meiazero/crosgha/commit/9fdc4d22cc15228f9cdc6d4500e7c6dfe0d1f4e8))
* describe the flat workflow scan and the X64 runner requirement ([95289cd](https://github.com/meiazero/crosgha/commit/95289cd5cb210f7607a87a012c6e3a8768c42355))

## [0.11.1](https://github.com/meiazero/crosgha/compare/v0.11.0...v0.11.1) (2026-09-28)


### Fixed

* cut false positives found by the benchmark ([81f5ede](https://github.com/meiazero/crosgha/commit/81f5eded471d5b19e3b33a3dc279a5c546291a19))

## [0.11.0](https://github.com/meiazero/crosgha/compare/v0.10.0...v0.11.0) (2026-09-28)


### Added

* **rules:** flag actions/* versions built for a retired Node runtime ([3380ac4](https://github.com/meiazero/crosgha/commit/3380ac48d4691a45f1ed2591d49e91aa75b31813))


### Fixed

* **rules:** catch moving refs and conda installs in unpinned-package ([1af2c22](https://github.com/meiazero/crosgha/commit/1af2c22725835474a32fa5df601bfa65d63934dd))
* **rules:** keep local hosts out of insecure-protocol ([f3f38aa](https://github.com/meiazero/crosgha/commit/f3f38aa8ece0c76e7198320a09de3f3fe29b0a84))
* **rules:** spare workflow-level scopes that only one job inherits ([22c24c4](https://github.com/meiazero/crosgha/commit/22c24c452fc0ff952dd6f8a645712f84d0a66286))
* **workflow:** parse files that start with a UTF-8 BOM ([f3d9633](https://github.com/meiazero/crosgha/commit/f3d9633964a5542e8323de566b74134eeeea821d))


### Changed

* parallel scan and rule fixes from the benchmark ([f96e87a](https://github.com/meiazero/crosgha/commit/f96e87a7ead4ac387f1b5cf903c81895f8124ba8))
* **scan:** inspect workflows and repository dirs in parallel ([403b6d3](https://github.com/meiazero/crosgha/commit/403b6d3f23905aba804570c59012bbb47643ef6f))

## [0.10.0](https://github.com/meiazero/crosgha/compare/v0.9.0...v0.10.0) (2026-09-27)


### Added

* **install:** add a curl | sh installer for the release binary ([34a3197](https://github.com/meiazero/crosgha/commit/34a3197beae98ed8c8aad37f3afece1d052680d5))
* **install:** add a curl | sh installer for the release binary ([4f90273](https://github.com/meiazero/crosgha/commit/4f90273b8fb7cd6905c59b0a04faf87899448133))

## [0.9.0](https://github.com/meiazero/crosgha/compare/v0.8.1...v0.9.0) (2026-09-27)


### Added

* render text findings as source diagnostics with color ([26591cf](https://github.com/meiazero/crosgha/commit/26591cfb156ca6333f67f07f4a5b60ee594491ee))
* **report:** render text findings as source diagnostics with color ([f5183ec](https://github.com/meiazero/crosgha/commit/f5183ecc842d45c47ccbc3436ea3406c98c2a9ea))

## [0.8.1](https://github.com/meiazero/crosgha/compare/v0.8.0...v0.8.1) (2026-09-27)


### Fixed

* audit a dependabot.yml input instead of parsing it as a workflow ([6d2692b](https://github.com/meiazero/crosgha/commit/6d2692b2e0dc5dfa7ce09ea11f0341c4ab498dd3))
* handle dependabot.yml and .github inputs ([e4a6710](https://github.com/meiazero/crosgha/commit/e4a67106a55849abccb396e44ae2f9f0bd63ee0a))
* scan a .github input through its workflows directory ([7e6ab05](https://github.com/meiazero/crosgha/commit/7e6ab0564eba73ce09ebe4fed64e325fe1e8589c))

## [0.8.0](https://github.com/meiazero/crosgha/compare/v0.7.1...v0.8.0) (2026-09-27)


### Added

* report parse failures in JSON and SARIF and scope github.com checks to github.com ([507a9d4](https://github.com/meiazero/crosgha/commit/507a9d42d2bb4f725b2aecf65c4519d9704c65c5))


### Fixed

* **hook:** refuse a foreign pre-push hook that cannot be read ([16b1464](https://github.com/meiazero/crosgha/commit/16b14646c3785ecf53068b5084731ab657d87726))
* review follow-ups for the hook, removed-feature, and the restructured docs ([c3cd531](https://github.com/meiazero/crosgha/commit/c3cd5318180faa12b9a78b5cbb4221de26893edb))
* **rules:** resolve matrix runners and skip self-hosted labels in removed-feature ([9488b38](https://github.com/meiazero/crosgha/commit/9488b38e0937c234894f90e135d6a2c919de8fa4))


### Documentation

* drop the duplicated PR-mode sections and the stale parse-error claim ([909e3b3](https://github.com/meiazero/crosgha/commit/909e3b3caabea38854c2981b57d331a04db96f43))

## [0.7.1](https://github.com/meiazero/crosgha/compare/v0.7.0...v0.7.1) (2026-09-27)


### Documentation

* **agents:** list capabilities as bullets and fix layout entries ([0363439](https://github.com/meiazero/crosgha/commit/0363439830d4ee1e89b1764d7328b52258094eea))
* **architecture:** drop planned-action wording and fix stage order ([84c6f8e](https://github.com/meiazero/crosgha/commit/84c6f8e95ec05d3a43322a4b0e90368a56fd0ada))
* **readme:** reorder by user journey and add release install ([81e6d3a](https://github.com/meiazero/crosgha/commit/81e6d3ae60a900bdd873ce10574ad22bc3b69c17))
* restructure readme, roadmap, and architecture ([0f09644](https://github.com/meiazero/crosgha/commit/0f0964442bc2807b6841facf3f38edf38616aca7))
* **roadmap:** split shipped work from next steps ([ce544c7](https://github.com/meiazero/crosgha/commit/ce544c72a5892b1a11fb5db9ba8bf4b9ba47553c))

## [0.7.0](https://github.com/meiazero/crosgha/compare/v0.6.0...v0.7.0) (2026-09-27)


### Added

* add a pre-push hook and flag features GitHub removed ([c631212](https://github.com/meiazero/crosgha/commit/c6312120c77e3784a3704e9201522e92e50e9d8c))
* **cli:** add hook subcommand to install a pre-push scan ([2e1536d](https://github.com/meiazero/crosgha/commit/2e1536dba80737fb7a7a0fd052e69310b51e7e2d))
* **rules:** catch workflow validation errors in schema-validity ([8f47f04](https://github.com/meiazero/crosgha/commit/8f47f047fec1f5aa9dbf2780c16074953a1f7a92))
* **rules:** flag retired runners and shut-down action versions ([b8d40ba](https://github.com/meiazero/crosgha/commit/b8d40ba97fd908b5779ef81392756203a38e95f3))

## [0.6.0](https://github.com/meiazero/crosgha/compare/v0.5.0...v0.6.0) (2026-09-27)


### Added

* flag the removed node version opt-out variable ([c656de5](https://github.com/meiazero/crosgha/commit/c656de5e725c7c4d05ca11b1cb3a8315975a52b2))

## [0.5.0](https://github.com/meiazero/crosgha/compare/v0.4.0...v0.5.0) (2026-09-27)


### Added

* add retired-runtime for actions on retired node runtimes ([e55ba5d](https://github.com/meiazero/crosgha/commit/e55ba5d811ed6f04b4e747f18e58c385374966ae))

## [0.4.0](https://github.com/meiazero/crosgha/compare/v0.3.0...v0.4.0) (2026-09-27)


### Added

* flag uncapped reusable workflow calls on outsider triggers ([868e529](https://github.com/meiazero/crosgha/commit/868e529231b8d1be556d73851eb02e033a0a63d6))
* flag unknown GITHUB_TOKEN permission scopes ([29dfd84](https://github.com/meiazero/crosgha/commit/29dfd84b6fd6f4cf82ca2e6e7bcecf5f851c16fb))

## [0.3.0](https://github.com/meiazero/crosgha/compare/v0.2.0...v0.3.0) (2026-09-27)


### Added

* **rules:** recognize setup-dotnet's cache input in cache-misuse ([0d008c8](https://github.com/meiazero/crosgha/commit/0d008c8a9244b0cc5ab718731c4ad64e7e038de0))
* **rules:** treat fork and watch as outsider-triggered events ([8d62380](https://github.com/meiazero/crosgha/commit/8d62380fab4fee413317ba45143b34162e9afac6))


### Fixed

* **cli:** keep reporting other workflows when one fails to parse ([8462dd1](https://github.com/meiazero/crosgha/commit/8462dd1ad3afcb93ac398a008b4aca55ee879cbb))
* **rules:** accept github.event.repository fork guards in missing-fork-guard ([59dc8d9](https://github.com/meiazero/crosgha/commit/59dc8d9b6072b9fa7fdfb0e3bcec088149efe8ba))
* **rules:** count Dependabot's 3-day default in missing-release-cooldown ([0723565](https://github.com/meiazero/crosgha/commit/07235658d9dc1a6a9431eddc6b995769aadadcbd))

## [0.2.0](https://github.com/meiazero/crosgha/compare/v0.1.0...v0.2.0) (2026-09-27)


### ⚠ BREAKING CHANGES

* **report:** add agent format and default to annotations in actions

### Added

* **report:** add agent format and default to annotations in actions ([9d840d5](https://github.com/meiazero/crosgha/commit/9d840d586d3c5fad6c94cd025c5d6999934b041d))


### Documentation

* document the agent format and the annotations default ([73cf8a1](https://github.com/meiazero/crosgha/commit/73cf8a13aa2ed5a6320babba1edda48688f77eec))

## 0.1.0 (2026-09-27)


### ⚠ BREAKING CHANGES

* **action:** accept zizmor-action inputs
* cover the smell taxonomy with zizmor-compatible rules
* rename package and binary to crosgha

### Added

* **action:** accept zizmor-action inputs ([4541eb6](https://github.com/meiazero/crosgha/commit/4541eb6111629c1093c7d1ec74bf3669905cda28))
* **action:** install the verified release and upload SARIF ([9af0de7](https://github.com/meiazero/crosgha/commit/9af0de7e38096bac0cf1cb88a324fffd580eb4e3))
* add rule options, JSON/SARIF output, and config migration ([07754cd](https://github.com/meiazero/crosgha/commit/07754cd71b8d6714508bfb8561242a060e227204))
* **cli:** scan workflow files and check top-level on and jobs ([a728b89](https://github.com/meiazero/crosgha/commit/a728b8955832251f14a7dcafe950824cd048523d))
* cover the smell taxonomy with zizmor-compatible rules ([828388e](https://github.com/meiazero/crosgha/commit/828388ee094f7f461fe2f198e3062da7f5aa8ad2))
* **rules:** add opt-in workflow policy checks ([3fa895f](https://github.com/meiazero/crosgha/commit/3fa895f01e44cda8d159a26ffee26af6257e82e4))
* **tool:** structure workflow analysis and action scaffold ([6217d74](https://github.com/meiazero/crosgha/commit/6217d740814cc543400ff6f6870592b87408f83b))


### Fixed

* **cli:** accept --version again ([54938e6](https://github.com/meiazero/crosgha/commit/54938e6b5c48b14cbb62274449999c9b6e8262fc))
* **hooks:** run pre-push under the mise toolchain ([315d13e](https://github.com/meiazero/crosgha/commit/315d13ee77ccfe99ae022d77a78fccfd81822f0e))
* **release:** tag draft releases so release-please finds the last one ([158b867](https://github.com/meiazero/crosgha/commit/158b867b1a2236e547ab04d8544dc431f1c77cd1))


### Changed

* rename package and binary to crosgha ([0bbe5aa](https://github.com/meiazero/crosgha/commit/0bbe5aac88fe4c52e58c95e47701c464a3acd7f2))


### Documentation

* add readme, roadmap and project guide ([e2a22fc](https://github.com/meiazero/crosgha/commit/e2a22fc8ba6c8fb94c5592662a28d3162cae327b))
* **claude:** describe the scanner and add testing rules ([d20c8fc](https://github.com/meiazero/crosgha/commit/d20c8fc6f613b0711dd2e40a29bf36e0fe25e00d))
* describe personas, compatibility, and the full rule set ([4c3541f](https://github.com/meiazero/crosgha/commit/4c3541f8b6239927abc1daa1f5005f3a3db91bfa))
* document rule options, report formats, and the Action ([933f3ab](https://github.com/meiazero/crosgha/commit/933f3ab57ccea4ee7a4655b838fa7bf9d899b385))
* drop references to the removed releasing guide ([b316f12](https://github.com/meiazero/crosgha/commit/b316f121b496de9b1af8d9f06a7f4aedf1a69699))
* record the GitHub repo and refine the README ([6b07d5d](https://github.com/meiazero/crosgha/commit/6b07d5defb2a007f6e26eb71be1268596086a521))
* remove releasing guide and roadmap ([a6543af](https://github.com/meiazero/crosgha/commit/a6543af04f0a6ad11fe16662f428e48986497516))
* restore roadmap with the crosgha name ([d63b273](https://github.com/meiazero/crosgha/commit/d63b273fc7dd42ec0af7b7e193d201bb920c060f))

## Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Entries below this line are managed by [release-please](https://github.com/googleapis/release-please);
do not edit them by hand.
