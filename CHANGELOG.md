# [1.10.0](https://github.com/misty-step/landfall/compare/v1.9.0...v1.10.0) (2026-02-11)


### Features

* auto-close stale synthesis failure issues on success ([#73](https://github.com/misty-step/landfall/issues/73)) ([b276399](https://github.com/misty-step/landfall/commit/b2763993c9b959f9dc85fb7a8b502a150f404223)), closes [#45](https://github.com/misty-step/landfall/issues/45)

# [1.9.0](https://github.com/misty-step/landfall/compare/v1.8.0...v1.9.0) (2026-02-11)


### Bug Fixes

* don't create failure issues when API key is unconfigured ([#67](https://github.com/misty-step/landfall/issues/67)) ([30adeb0](https://github.com/misty-step/landfall/commit/30adeb07c79a2d91dabc83460c109cfda08b4de2)), closes [#46](https://github.com/misty-step/landfall/issues/46)
* surface actionable diagnosis for 401/403 LLM API errors ([#68](https://github.com/misty-step/landfall/issues/68)) ([c893b9c](https://github.com/misty-step/landfall/commit/c893b9c08302ae9588faaea9f012011c58dee7b8)), closes [#47](https://github.com/misty-step/landfall/issues/47)


### Features

* support custom synthesis prompt templates ([#66](https://github.com/misty-step/landfall/issues/66)) ([8d022d1](https://github.com/misty-step/landfall/commit/8d022d13c8b3d3ffe9fe598ffc5ba3f94fe380a5)), closes [#15](https://github.com/misty-step/landfall/issues/15)

# [1.8.0](https://github.com/misty-step/landfall/compare/v1.7.0...v1.8.0) (2026-02-11)


### Features

* add proactive API key health check ([#69](https://github.com/misty-step/landfall/issues/69)) ([7d943cb](https://github.com/misty-step/landfall/commit/7d943cbc7170da281ba4e304ab6afcd8b3e06c9b)), closes [#49](https://github.com/misty-step/landfall/issues/49)
* add synthesis-only mode to decouple from semantic-release ([#70](https://github.com/misty-step/landfall/issues/70)) ([3fa37e1](https://github.com/misty-step/landfall/commit/3fa37e109409bb8684760c8cb81dbdd21dff8fc2)), closes [#50](https://github.com/misty-step/landfall/issues/50)
* rewrite synthesis system message and prompt template ([#71](https://github.com/misty-step/landfall/issues/71)) ([04822aa](https://github.com/misty-step/landfall/commit/04822aa8b299264b1a4d620843e68f5aeda83c27)), closes [#53](https://github.com/misty-step/landfall/issues/53)

# [1.7.0](https://github.com/misty-step/landfall/compare/v1.6.1...v1.7.0) (2026-02-11)


### Features

* support consuming repo .releaserc override ([#65](https://github.com/misty-step/landfall/issues/65)) ([a173a38](https://github.com/misty-step/landfall/commit/a173a38142e102624e76290bfabeb651391de2a1)), closes [#14](https://github.com/misty-step/landfall/issues/14)

## [1.6.1](https://github.com/misty-step/landfall/compare/v1.6.0...v1.6.1) (2026-02-11)


### Bug Fixes

* make action dependency installs deterministic ([#42](https://github.com/misty-step/landfall/issues/42)) ([31d06ca](https://github.com/misty-step/landfall/commit/31d06ca30e471771e56d11ae3272c185250f8467)), closes [#35](https://github.com/misty-step/landfall/issues/35)

# [1.6.0](https://github.com/misty-step/landfall/compare/v1.5.0...v1.6.0) (2026-02-10)


### Features

* add floating major version tag support for GitHub Actions repos ([#28](https://github.com/misty-step/landfall/issues/28)) ([#40](https://github.com/misty-step/landfall/issues/40)) ([7cbb7a0](https://github.com/misty-step/landfall/commit/7cbb7a0e73f6057553cbde7a312407070e468517))

# [1.5.0](https://github.com/misty-step/landfall/compare/v1.4.0...v1.5.0) (2026-02-10)


### Features

* backfill CLI for retroactive release note synthesis ([#39](https://github.com/misty-step/landfall/issues/39)) ([8b57870](https://github.com/misty-step/landfall/commit/8b57870dfdf3fe85147be24758aa59e224601e11)), closes [#13](https://github.com/misty-step/landfall/issues/13) [#13](https://github.com/misty-step/landfall/issues/13)

# [1.4.0](https://github.com/misty-step/landfall/compare/v1.3.4...v1.4.0) (2026-02-10)


### Features

* add plaintext/html artifact outputs ([#12](https://github.com/misty-step/landfall/issues/12)) ([#38](https://github.com/misty-step/landfall/issues/38)) ([dcfb14f](https://github.com/misty-step/landfall/commit/dcfb14f21924a7a5686c2d27338379dd29c45bb3))

## [1.3.4](https://github.com/misty-step/landfall/compare/v1.3.3...v1.3.4) (2026-02-10)


### Bug Fixes

* **ci:** correct test directory paths and add ruff config ([#33](https://github.com/misty-step/landfall/issues/33)) ([0f795e0](https://github.com/misty-step/landfall/commit/0f795e0ca758e5581379a364c46d3495172454f3)), closes [#11](https://github.com/misty-step/landfall/issues/11)

## [1.3.3](https://github.com/misty-step/landfall/compare/v1.3.2...v1.3.3) (2026-02-10)


### Bug Fixes

* **ci:** add dedicated workflow to sync v1 tag ([#32](https://github.com/misty-step/landfall/issues/32)) ([04c91c5](https://github.com/misty-step/landfall/commit/04c91c55c9740f05ceefa7f063e469b2bcf1211d)), closes [#16](https://github.com/misty-step/landfall/issues/16)

## [1.3.2](https://github.com/misty-step/landfall/compare/v1.3.1...v1.3.2) (2026-02-10)


### Bug Fixes

* fall back to GitHub release body when CHANGELOG.md is missing ([#31](https://github.com/misty-step/landfall/issues/31)) ([1384cf5](https://github.com/misty-step/landfall/commit/1384cf50a3d237ab4651ff2e662d4ca8734e33af)), closes [misty-step/cerberus#82](https://github.com/misty-step/cerberus/issues/82)

## [1.3.1](https://github.com/misty-step/landfall/compare/v1.3.0...v1.3.1) (2026-02-09)


### Bug Fixes

* keep changelog semantic-release only ([#27](https://github.com/misty-step/landfall/issues/27)) ([5f9a235](https://github.com/misty-step/landfall/commit/5f9a2352c379b82687fa997d214a50cdfcd2ee94))

# [1.3.0](https://github.com/misty-step/landfall/compare/v1.2.0...v1.3.0) (2026-02-09)


### Features

* generate portable release notes artifacts ([#26](https://github.com/misty-step/landfall/issues/26)) ([d4ca901](https://github.com/misty-step/landfall/commit/d4ca90199c4b022407ee8ba2705d2a385100356f)), closes [#7](https://github.com/misty-step/landfall/issues/7)

# [1.2.0](https://github.com/misty-step/landfall/compare/v1.1.5...v1.2.0) (2026-02-09)


### Features

* alert and signal synthesis failures ([#25](https://github.com/misty-step/landfall/issues/25)) ([8398ca0](https://github.com/misty-step/landfall/commit/8398ca066c51c45a025adff1e536c0bbdf2d5202))

## [1.1.5](https://github.com/misty-step/landfall/compare/v1.1.4...v1.1.5) (2026-02-09)


### Bug Fixes

* remove unused @semantic-release/npm dependency ([#24](https://github.com/misty-step/landfall/issues/24)) ([a353646](https://github.com/misty-step/landfall/commit/a353646e21c3381e440536e1c3ab3435dbeb3959)), closes [#5](https://github.com/misty-step/landfall/issues/5)

## [1.1.4](https://github.com/misty-step/landfall/compare/v1.1.3...v1.1.4) (2026-02-09)


### Bug Fixes

* harden self-release notes pipeline ([#23](https://github.com/misty-step/landfall/issues/23)) ([0a030b4](https://github.com/misty-step/landfall/commit/0a030b4c21e88daf5b5d68fd75cae2b83ce9938f))

## [1.1.3](https://github.com/misty-step/landfall/compare/v1.1.2...v1.1.3) (2026-02-08)


### Bug Fixes

* remove dead backward-compat code and warn on insecure API URLs ([#20](https://github.com/misty-step/landfall/issues/20)) ([0df6c21](https://github.com/misty-step/landfall/commit/0df6c21d601c60e71c25a86184b4ac67499535d4)), closes [#3](https://github.com/misty-step/landfall/issues/3) [#4](https://github.com/misty-step/landfall/issues/4)

## [1.1.2](https://github.com/misty-step/landfall/compare/v1.1.1...v1.1.2) (2026-02-08)


### Bug Fixes

* provider-agnostic LLM inputs for release synthesis ([#19](https://github.com/misty-step/landfall/issues/19)) ([451db2a](https://github.com/misty-step/landfall/commit/451db2a01256030bedd0039396af86e6f6a5ac03)), closes [#4](https://github.com/misty-step/landfall/issues/4)

## [1.1.1](https://github.com/misty-step/landfall/compare/v1.1.0...v1.1.1) (2026-02-08)


### Bug Fixes

* remove npm plugin and package.json references for non-Node project support ([#17](https://github.com/misty-step/landfall/issues/17)) ([c5e9dc0](https://github.com/misty-step/landfall/commit/c5e9dc0257622fff7914ac2db49732d764c39296)), closes [misty-step/vox#178](https://github.com/misty-step/vox/issues/178)

# [1.1.0](https://github.com/misty-step/landfall/compare/v1.0.0...v1.1.0) (2026-02-08)


### Bug Fixes

* harden release workflow template (concurrency, timeout, docs) ([#2](https://github.com/misty-step/landfall/issues/2)) ([228f57f](https://github.com/misty-step/landfall/commit/228f57f67cb93db2d7b4d9ebfed6a4a485f330e3))


### Features

* integrate Landfall release pipeline ([#1](https://github.com/misty-step/landfall/issues/1)) ([2d36967](https://github.com/misty-step/landfall/commit/2d36967ac612d228fa03905bc664cc4af74cd1d1))
