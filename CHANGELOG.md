# Changelog

## [0.21.0](https://github.com/reset-ufc/crosgha/compare/v0.20.0...v0.21.0) (2026-11-14)


### Added

* **rules:** report gh steps without the token, host, or repository they need ([#158](https://github.com/reset-ufc/crosgha/issues/158)) ([52368c6](https://github.com/reset-ufc/crosgha/commit/52368c678d2c94399c15f8f1771b8d896bd2ab81)), closes [#88](https://github.com/reset-ufc/crosgha/issues/88)

## [0.20.0](https://github.com/reset-ufc/crosgha/compare/v0.19.0...v0.20.0) (2026-11-14)


### Added

* **rules:** report and fix actions that pass when they find nothing ([#156](https://github.com/reset-ufc/crosgha/issues/156)) ([8b73d2a](https://github.com/reset-ufc/crosgha/commit/8b73d2ab0391949c4f59f6618e45ebc322751e9f)), closes [#87](https://github.com/reset-ufc/crosgha/issues/87)

## [0.19.0](https://github.com/reset-ufc/crosgha/compare/v0.18.0...v0.19.0) (2026-11-14)


### Added

* **rules:** fix mixed if conditions and report conditions that hide their intent ([#154](https://github.com/reset-ufc/crosgha/issues/154)) ([f7ccb8e](https://github.com/reset-ufc/crosgha/commit/f7ccb8ee8a8c286c6bf70594043f6c032ae6336b)), closes [#89](https://github.com/reset-ufc/crosgha/issues/89) [#75](https://github.com/reset-ufc/crosgha/issues/75)

## [0.18.0](https://github.com/reset-ufc/crosgha/compare/v0.17.0...v0.18.0) (2026-11-14)


### Added

* **rules:** report environment file overwrites and unbraced paths ([#152](https://github.com/reset-ufc/crosgha/issues/152)) ([f909b30](https://github.com/reset-ufc/crosgha/commit/f909b30393b9d9f966e90d9bb511742f99e5e08e)), closes [#86](https://github.com/reset-ufc/crosgha/issues/86)

## [0.17.0](https://github.com/reset-ufc/crosgha/compare/v0.16.0...v0.17.0) (2026-11-14)


### Added

* **fix:** quote environment file paths in run scripts ([ca1727f](https://github.com/reset-ufc/crosgha/commit/ca1727f8ff0d46dd5ff33fec850769fefa3a7bf1))
* **fix:** quote environment file paths in run scripts ([e98c98c](https://github.com/reset-ufc/crosgha/commit/e98c98c38d145d52856d580400d7b3f80611577d))
* **fix:** rewrite deprecated workflow commands to environment files ([a05e08c](https://github.com/reset-ufc/crosgha/commit/a05e08c4a95f89675750b2b039191703c71a5c13))
* **fix:** rewrite deprecated workflow commands to environment files ([930d922](https://github.com/reset-ufc/crosgha/commit/930d922f008a98e04a20aa893d7322206b1df572)), closes [#74](https://github.com/reset-ufc/crosgha/issues/74)


### Fixed

* **fix:** leave environment file paths in shell patterns unquoted ([2ba8914](https://github.com/reset-ufc/crosgha/commit/2ba891466842b68184a3de45f675b4d43d416a96))
* **fix:** quote only environment file paths that are redirect targets ([20c8a42](https://github.com/reset-ufc/crosgha/commit/20c8a4204744360f9f4b84c38df1162f9748b7b1))
* **fix:** write add-path rewrites with printf so `-n` paths survive ([1ef1e62](https://github.com/reset-ufc/crosgha/commit/1ef1e6268da43d01cfaa10770e17273c1043bd88))

## [0.16.0](https://github.com/reset-ufc/crosgha/compare/v0.15.1...v0.16.0) (2026-11-13)


### Added

* **report:** name ghlint equivalents in reports and the rule catalog ([6b0d821](https://github.com/reset-ufc/crosgha/commit/6b0d821f823a8fb82b29be810b4cbc1c7fe146c9))
* **report:** name ghlint equivalents in reports and the rule catalog ([7becf33](https://github.com/reset-ufc/crosgha/commit/7becf33a783abfd07f53caf40046e1ca0bb18e7e)), closes [#92](https://github.com/reset-ufc/crosgha/issues/92)

## [0.15.1](https://github.com/reset-ufc/crosgha/compare/v0.15.0...v0.15.1) (2026-11-02)


### Changed

* follow the rust style guide in existing code ([19783f1](https://github.com/reset-ufc/crosgha/commit/19783f1adb677bde3740f35172e0093587bb4fb9))
* import items per module ([963ea38](https://github.com/reset-ufc/crosgha/commit/963ea38e611c125cc19bdcfa7e20cd0a27db8e13))


### Documentation

* adopt the google rust style guide ([a8aa832](https://github.com/reset-ufc/crosgha/commit/a8aa832c75b87a3b90e6fde3deea560bfcf00afa))
* require the rust style guide for every rust change ([a3ce42b](https://github.com/reset-ufc/crosgha/commit/a3ce42b9c234abc1d94be91f18ac3d9359e2e641))

## [0.15.0](https://github.com/reset-ufc/crosgha/compare/v0.14.1...v0.15.0) (2026-11-02)


### Added

* scan remote repository workflows ([6a231bd](https://github.com/reset-ufc/crosgha/commit/6a231bd18cf5ea393e273614ee4c46909b6e665c))
* **scan:** read workflows from remote repositories ([1c8c196](https://github.com/reset-ufc/crosgha/commit/1c8c1964962860dd25268f32e9b23e99f43567cb))


### Fixed

* **scan:** keep missing local paths out of remote inputs ([b59cb1c](https://github.com/reset-ufc/crosgha/commit/b59cb1ca027740e433db52851198adcd5a628cf1))

## [0.14.1](https://github.com/reset-ufc/crosgha/compare/v0.14.0...v0.14.1) (2026-11-01)


### Fixed

* **rules:** read --from and --with as packages in uv tool install ([e26ba96](https://github.com/reset-ufc/crosgha/commit/e26ba96276b39a2680c5265a03eb7838c57dfa30))
* **rules:** read --from and --with as packages in uv tool install ([a5d33ae](https://github.com/reset-ufc/crosgha/commit/a5d33ae6609e20466dd155d51dd0a71dd18cbfa1))

## [0.14.0](https://github.com/reset-ufc/crosgha/compare/v0.13.4...v0.14.0) (2026-11-01)


### Added

* **rules:** report workflows GitHub refuses to run ([2351956](https://github.com/reset-ufc/crosgha/commit/2351956678b055c4768bf40ecde1d4f315b12810))
* **rules:** report workflows GitHub refuses to run ([80a7c0b](https://github.com/reset-ufc/crosgha/commit/80a7c0b67a29dc17b2dc59c0cbc30a4d90d472ec))

## [0.13.4](https://github.com/reset-ufc/crosgha/compare/v0.13.3...v0.13.4) (2026-11-01)


### Fixed

* **rules:** skip option values of yarn and uv tool in unpinned-package ([bb0edc2](https://github.com/reset-ufc/crosgha/commit/bb0edc2f6f7ab59c2ab1660e2d9577c0375f2bdd))
* **rules:** skip option values of yarn and uv tool in unpinned-package ([24936ff](https://github.com/reset-ufc/crosgha/commit/24936ff6e711c75cdca9f6e9a710e8eb55c7908d))

## [0.13.3](https://github.com/reset-ufc/crosgha/compare/v0.13.2...v0.13.3) (2026-11-01)


### Fixed

* **action:** keep the job summary free of workflow commands and ANSI codes ([593765b](https://github.com/reset-ufc/crosgha/commit/593765bbeb5493993f6d1b3b776eca7d2cd739dc))
* **action:** keep the job summary free of workflow commands and ANSI codes ([58531b7](https://github.com/reset-ufc/crosgha/commit/58531b7e17b5f62fe6d99d7adee77f7655b605ef))
* **config:** warn about rule IDs that no rule answers to ([d5dce65](https://github.com/reset-ufc/crosgha/commit/d5dce65dc557a0c62d7b4f256869d5aff567c674))
* **config:** warn about rule IDs that no rule answers to ([1c1d56c](https://github.com/reset-ufc/crosgha/commit/1c1d56c104de7ef86307beedfbf7bf7bf93b2e1a))
* **install:** point the documented URL at a host that serves it ([063b207](https://github.com/reset-ufc/crosgha/commit/063b20749fcd9defb645741388b46ffc9b7fcd22))
* **install:** point the documented URL at a host that serves it ([686ebcb](https://github.com/reset-ufc/crosgha/commit/686ebcbc4d2350ca75df1be9160a51ba65861795))
* **rules:** close version and text matching gaps in three rules ([c8a1f47](https://github.com/reset-ufc/crosgha/commit/c8a1f47739568dfc19e4c1592611ed72197ba2ad))
* **rules:** close version and text matching gaps in three rules ([6fa4356](https://github.com/reset-ufc/crosgha/commit/6fa4356def1452bf83387f1631defa9468ef993c))
* **rules:** read version flags and global options in unpinned-package ([58e3399](https://github.com/reset-ufc/crosgha/commit/58e3399249c31bcd2565a11254e7d3d3a9fd137f))
* **rules:** read version flags and global options in unpinned-package ([a375258](https://github.com/reset-ufc/crosgha/commit/a375258ba95f56725e95406f3c360fe8ae8b4f82))
* **rules:** stop reporting expressions that only return true or false ([280fc25](https://github.com/reset-ufc/crosgha/commit/280fc25ecabfb46b2769187b0fe2692f763a70f8))
* **rules:** stop reporting expressions that only return true or false ([a8c44da](https://github.com/reset-ufc/crosgha/commit/a8c44da69e0e9ac4527e398a6d2900d24830c82c))


### Documentation

* **agents:** list the crates Cargo.toml actually depends on ([4b98e8a](https://github.com/reset-ufc/crosgha/commit/4b98e8a4eba226f10c8497407204b8a6a5506375))
* **agents:** list the crates Cargo.toml actually depends on ([3a10fd6](https://github.com/reset-ufc/crosgha/commit/3a10fd6fa2243a27266bc6bdf83cd5abb93cd5f4))
* **rules:** add non-compliant and compliant examples for every rule ([70eff61](https://github.com/reset-ufc/crosgha/commit/70eff617b6d4a0e17e6fe1a12913556bc59a0094))
* **rules:** add non-compliant and compliant examples for every rule ([1f0e482](https://github.com/reset-ufc/crosgha/commit/1f0e48211c8b67f020965590f80bba8db7e8ae40))
* **rules:** regenerate rule examples from the merged fixtures ([62d44f7](https://github.com/reset-ufc/crosgha/commit/62d44f71c8bb3c21332cc5563fea76da1d3d424d))

## [0.13.2](https://github.com/reset-ufc/crosgha/compare/v0.13.1...v0.13.2) (2026-10-28)


### Fixed

* **action:** keep the findings of other inputs when one has no workflows ([0033991](https://github.com/reset-ufc/crosgha/commit/0033991292773bc4f2f54252bbfe1f17af2b80a5))
* **action:** keep the findings of other inputs when one has no workflows ([205beb5](https://github.com/reset-ufc/crosgha/commit/205beb5f7b1c921d156e1a397d9c64efb61e62d8)), closes [#105](https://github.com/reset-ufc/crosgha/issues/105)

## [0.13.1](https://github.com/reset-ufc/crosgha/compare/v0.13.0...v0.13.1) (2026-10-28)


### Fixed

* **install:** stop running the downloaded binary before installing it ([b3fb2d1](https://github.com/reset-ufc/crosgha/commit/b3fb2d1f0beabc6f611c8dad8994e985cc811974))
* **install:** stop running the downloaded binary before installing it ([0463510](https://github.com/reset-ufc/crosgha/commit/0463510ab58dfcf8e5471b8a0351198896705c80))

## [0.13.0](https://github.com/reset-ufc/crosgha/compare/v0.12.2...v0.13.0) (2026-10-28)


### ⚠ BREAKING CHANGES

* **release:** the release asset is now crosgha-x86_64-unknown-linux-musl.tar.xz; scripts that download crosgha-x86_64-unknown-linux-gnu.tar.xz by name must switch.

### Added

* **release:** attest build provenance and check the tag version ([121b788](https://github.com/reset-ufc/crosgha/commit/121b788656a7c6d0d9159c94304a0b6048fd14d0))
* **release:** ship a static musl binary instead of the glibc one ([8d82750](https://github.com/reset-ufc/crosgha/commit/8d827502115faf69affb7ccb9c2f1806db89a85c))


### Fixed

* **deps:** update rustls and webpki crates for RustSec advisories ([da917f6](https://github.com/reset-ufc/crosgha/commit/da917f69c3a1c79d59abe525f48b0dda9fa32c9e))


### Documentation

* **agents:** document the new checks and the third-party test rule ([2c4123b](https://github.com/reset-ufc/crosgha/commit/2c4123b8b54067eb679b4dc5342d59cd4273aa7b))
* **agents:** mark the repository as public ([29ee7dd](https://github.com/reset-ufc/crosgha/commit/29ee7ddcded8e19c70bc9c5c1f56b755c8e953e0))

## [0.12.2](https://github.com/reset-ufc/crosgha/compare/v0.12.1...v0.12.2) (2026-10-28)


### Documentation

* track planned work in issues instead of ROADMAP.md ([ad5827a](https://github.com/reset-ufc/crosgha/commit/ad5827a4c88866b01220783235bee5f7110ab065))

## [0.12.1](https://github.com/reset-ufc/crosgha/compare/v0.12.0...v0.12.1) (2026-10-28)


### Fixed

* follow the move to reset-ufc and split the README ([1f8bd4a](https://github.com/reset-ufc/crosgha/commit/1f8bd4a1352896a5755312329b3d4985386ff55d))
* point repository and install URLs at reset-ufc ([00d481b](https://github.com/reset-ufc/crosgha/commit/00d481bbc8b137e7c9800cd12b532e2ff65bd057))


### Documentation

* **agents:** list allowed crates without versions ([3d32c9a](https://github.com/reset-ufc/crosgha/commit/3d32c9a46867ebc7f84579108ea8f9e0353349fe))
* split the README into topic pages under docs/ ([6d5d1e9](https://github.com/reset-ufc/crosgha/commit/6d5d1e9244ba778dfac6421f39cbfd3dea7bcaba))

## [0.12.0](https://github.com/meiazero/crosgha/compare/v0.11.2...v0.12.0) (2026-10-03)


### Added

* **cli:** accept zizmor's --pedantic flag and honor FORCE_COLOR ([a980967](https://github.com/meiazero/crosgha/commit/a980967b262c6b6b346e8bc307f4ebdc7cb5530c))
* **report:** mark configurable rules in the JSON rule catalog ([5e61fb8](https://github.com/meiazero/crosgha/commit/5e61fb878a4a9d127b215ef4cca9a1e5fbe9d49c))


### Fixed

* **action:** print error annotations on stdout ([630dac9](https://github.com/meiazero/crosgha/commit/630dac97afe538a199670c10ceea0b9cb825bf69))
* **cli:** handle stdout write failures in hook and migrate output ([05d207b](https://github.com/meiazero/crosgha/commit/05d207b4a3e178bc9caebb58937714b798a6c39c))
* **cli:** handle stdout write failures instead of panicking ([442975e](https://github.com/meiazero/crosgha/commit/442975e412150ba80243d956bd4e34b62f021092))
* **cli:** scan a symlink to another scanned workflow only through its target ([00c350b](https://github.com/meiazero/crosgha/commit/00c350bc786ad5a908020996bef72098c8bd0f5c))
* **cli:** treat an empty CROSGHA_CONFIG or ZIZMOR_CONFIG as unset ([a9c0876](https://github.com/meiazero/crosgha/commit/a9c0876846d3e0e9328c7cbf790d85587463432a))
* **cli:** treat any spelling of github.com as the hosted API ([c019f8b](https://github.com/meiazero/crosgha/commit/c019f8bf6619ec6e8042e1f8a594b1236dbe7dbf))
* **config:** discover the config from the repository root for every input ([253a9c3](https://github.com/meiazero/crosgha/commit/253a9c34313f66643a34dbe36531e5544e97876b))
* **config:** reject wrong types, unknown keys, and unknown severities ([d6a5b6b](https://github.com/meiazero/crosgha/commit/d6a5b6b657d1bc56476b2f85d4a08e9b9565aa7b))
* **filesystem:** keep a workflow edited during the scan ([6256797](https://github.com/meiazero/crosgha/commit/6256797c00e87311d0a7088e26377f512866666d))
* **fix:** add block lines after trailing whitespace, not inside it ([aa8354e](https://github.com/meiazero/crosgha/commit/aa8354e3fbae66437652304e434ed2e310405e19))
* **fix:** apply fixes of equal steps under every persona ([283184c](https://github.com/meiazero/crosgha/commit/283184cf8ae95de3138d8a15d5f8001d5e546cb3))
* **fix:** keep each line's break in files with mixed line endings ([ca440e3](https://github.com/meiazero/crosgha/commit/ca440e3988b162ed61b731a465a7bb7dffe411c5))
* **github:** stop requesting after the token is rejected ([65f7c54](https://github.com/meiazero/crosgha/commit/65f7c5467ffa725ae6cfe680cbd4030d32e4e27f))
* **hook:** recognize only the exact generated script as crosgha's hook ([f2cbea6](https://github.com/meiazero/crosgha/commit/f2cbea615b3c43650adedb17c81d4c695f4574f0))
* **hook:** refuse to install into a hooks directory outside the repository ([8658c6b](https://github.com/meiazero/crosgha/commit/8658c6b7cb9dc7fbba8ec3c6e96df0c023ff1aa9))
* **hook:** reject hook arguments that edit files, skip the scan, or remove the hook ([f3fb0f8](https://github.com/meiazero/crosgha/commit/f3fb0f817f2b5d59f9e4f5e87815fce3fa67e5be))
* **migrate:** keep shell steps whose arguments carry shell syntax ([ea56563](https://github.com/meiazero/crosgha/commit/ea56563b8bd396c68611f5aaea02276cd1282993))
* **migrate:** never write a rule key twice into crosgha.yml ([346ef20](https://github.com/meiazero/crosgha/commit/346ef20d6225222b99a80da4d0491aa20075a405))
* **migrate:** refuse only symlinked workflows that need an edit ([ab6d2eb](https://github.com/meiazero/crosgha/commit/ab6d2eb4dc89dad909994669f406a76452be393b))
* **migrate:** remove install steps only from jobs that got a crosgha step ([0842ce1](https://github.com/meiazero/crosgha/commit/0842ce1f5078604fb2b0f6e8507256b6b82dc97b))
* **report:** count columns in characters and omit them on multi-line annotations ([e41d7ae](https://github.com/meiazero/crosgha/commit/e41d7ae0ee9aba34d31513968f195476a8d158f3))
* **report:** emit file URIs for SARIF paths outside the working directory ([e48d5c4](https://github.com/meiazero/crosgha/commit/e48d5c4d653f23e573b4c88ad095358718f42190))
* **report:** give repository-wide SARIF results a file location ([5a11daa](https://github.com/meiazero/crosgha/commit/5a11daa34aefff7ba5d14f1514627df64a31b899))
* **report:** give unapplied fixes the line of their reported finding ([a5920bc](https://github.com/meiazero/crosgha/commit/a5920bc56189ca26f2673ccaa1688e8ba0650b79))
* **report:** point repository-wide annotations at a workflow file ([bc92d78](https://github.com/meiazero/crosgha/commit/bc92d78300dd3e648c2b31e2a613730c2f83bc22))
* **report:** report incomplete scans and failed writes as run errors ([4320c78](https://github.com/meiazero/crosgha/commit/4320c78c2e96addb1dad25aeecafff61e100d942))
* **report:** show line 1 of a BOM file without the BOM ([d93ac6e](https://github.com/meiazero/crosgha/commit/d93ac6e60efeb411222e860f9ae759a09e0d5e03))
* **rules:** check permission scopes against GitHub's workflow schema ([b66ecce](https://github.com/meiazero/crosgha/commit/b66ecce6beba37f804bb2fd62eea76d613f7953b))
* **rules:** count setup-node v5 automatic caching as a cache save ([f0c9bf6](https://github.com/meiazero/crosgha/commit/f0c9bf6fbd4526ef185265785bac375100c6ea5a))
* **rules:** detect pull request fetches behind git global options ([9582c4f](https://github.com/meiazero/crosgha/commit/9582c4fe70b21aa3c62589b99d32bb5ed36a5c0b))
* **rules:** find the shell behind /usr/bin/env in custom shells ([ef99c4c](https://github.com/meiazero/crosgha/commit/ef99c4c040163d7f2f36785c7509acc5cd3ddd7d))
* **rules:** ignore bot actor checks negated around their group ([969666c](https://github.com/meiazero/crosgha/commit/969666c6c5c7f97cf4c3c6e2322e67810141c113))
* **rules:** ignore test conditions and test-tool installs in missing-matrix ([63adbd0](https://github.com/meiazero/crosgha/commit/63adbd03406fe4b0b9101aa99c8dbd75066588ef))
* **rules:** keep persist-credentials unfixed when the token is needed ([d068552](https://github.com/meiazero/crosgha/commit/d06855295daebb796aca197ca62a84abbeac090d))
* **rules:** match pull request head refs in index syntax and any case ([7ad57a0](https://github.com/meiazero/crosgha/commit/7ad57a0e85aaae7bfa00b86720e0e99fba8a1c86))
* **rules:** read conditions and run scripts through shared readers ([2ee5c23](https://github.com/meiazero/crosgha/commit/2ee5c233ae28b94aa38a89a58e7508e1998a4e33))
* **rules:** read expressions with }} in string literals and spaced indexes ([957856e](https://github.com/meiazero/crosgha/commit/957856e97166118c441e51e025e174747e8cc1a3))
* **rules:** read persist-credentials true without regard to case ([15ae64a](https://github.com/meiazero/crosgha/commit/15ae64acdb6b72a521d9242ec4ea6612739cdd9b))
* **rules:** read step ids from index syntax and only from evaluated text ([d7d61f0](https://github.com/meiazero/crosgha/commit/d7d61f0737a120e9496b91dbbb5303919d7df254))
* **rules:** report unsecure-command opt-outs written as TRUE or an expression ([a426da5](https://github.com/meiazero/crosgha/commit/a426da50fb8e60e10756d032a21da1b4d7b13230))
* **rules:** require an equality fork guard and stop inheriting it through success() || ([5568bf4](https://github.com/meiazero/crosgha/commit/5568bf403da33b7491f335e07ddfcb505734f26f))
* **rules:** resolve nested matrix paths in runs-on labels ([f73a682](https://github.com/meiazero/crosgha/commit/f73a6821e27aab342eb5e186af1d81f28f31e251))
* **rules:** stop reading quotes, option values, and expressions as packages ([f6db932](https://github.com/meiazero/crosgha/commit/f6db932d2395c8a440f44f35d7830ffab932634f))
* **rules:** stop reporting toJSON of scalar payload fields ([d955ff5](https://github.com/meiazero/crosgha/commit/d955ff5b93f9a3783937f589c71bc523839ebf44))
* **rules:** stop the tag walk when a page repeats or after 100 pages ([22fd0b0](https://github.com/meiazero/crosgha/commit/22fd0b0e684e1957bbea111f523d5607c795f2f6))
* **rules:** trace untrusted env values into environment file writes ([e028c67](https://github.com/meiazero/crosgha/commit/e028c67efc9c3b91e4c167eaec135706191b991f))
* **rules:** track saved remote scripts and read only an option's own letters ([22b69e8](https://github.com/meiazero/crosgha/commit/22b69e8c77905b4b0a1c1e0da4a2d64d1be62a74))
* **rules:** treat workflow_run display titles and edited text as untrusted ([4ca8e78](https://github.com/meiazero/crosgha/commit/4ca8e7845ea5ae9eb9ea948cacbf45207017fef6))
* second audit round (persist-credentials fix safety, reports, rules, config, hook) ([d733b12](https://github.com/meiazero/crosgha/commit/d733b12d5f3e2911484eee2ec58d8c7d3983af8d))


### Changed

* **config:** validate the config file with serde ([1fdf2a7](https://github.com/meiazero/crosgha/commit/1fdf2a7963198181514ce8dc4ddad77bd31d696c))
* **fix:** skip the fix pass when no finding proposes a fix ([2c60ab9](https://github.com/meiazero/crosgha/commit/2c60ab9ab5f87d7b1a6b42a9788419f974fd497a))
* **hook:** recognize every script a release has installed ([f14fe28](https://github.com/meiazero/crosgha/commit/f14fe28affe9bdcd7663dfb5099479533a9b03e4))
* **report:** decide where findings point before rendering ([64dd333](https://github.com/meiazero/crosgha/commit/64dd333aa35b1ff9e2c5431155d6c2168dda203e))


### Documentation

* describe the second audit's behavior changes ([f9b3b93](https://github.com/meiazero/crosgha/commit/f9b3b935f47b6cf6043ef2f292be0d456cfb3097))
* describe the shared readers and the split test files ([8b0f8e7](https://github.com/meiazero/crosgha/commit/8b0f8e79f05bccabdd917b5614136550c4c48528))

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
