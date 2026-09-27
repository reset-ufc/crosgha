# Changelog

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
