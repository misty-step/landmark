# [1.28.0](https://github.com/misty-step/landmark/compare/v1.27.0...v1.28.0) (2026-07-08)

### Features

* **release:** post release kit feed events (#192) ([eb0eb1d](https://github.com/misty-step/landmark/commit/eb0eb1d0cc163df3f5c14001f48fb2aead150a09))
* **release:** ground semver decisions in API evidence (#197) ([96bae1f](https://github.com/misty-step/landmark/commit/96bae1fa96eae6247d898ef6a5744159de5c269e))
* **release:** export static public release entries ([54bbf0e](https://github.com/misty-step/landmark/commit/54bbf0ef60f0490e626bc45647a227ba2fae4cbe))
* **release:** add gated social draft kit artifact ([ed677cf](https://github.com/misty-step/landmark/commit/ed677cf8cc66f714c108b0d3c8f6152b4270c029))
* **site:** add public marketing site from the Aesthetic site-kit (#201) ([0e0980f](https://github.com/misty-step/landmark/commit/0e0980f666f54a1deaae3b0f720a929c654b6fd1))
* **mcp:** landmark-mcp -- MCP face for the CLI's core read-only verbs (#202) ([ccfaf57](https://github.com/misty-step/landmark/commit/ccfaf57b48d0855830cff57f39bdeaf34b6b6a6e))
* **site:** lock landmark fleet marketing site (#203) ([d32110e](https://github.com/misty-step/landmark/commit/d32110ee3d68c87fb8ecd74902f43cbe3a64262c))
* **versioning:** pre-stable mode — 0.x stays 0.x until promoted (#204) ([3a69910](https://github.com/misty-step/landmark/commit/3a69910c350840bb702c2d8dc7e98bbe5ca501fd))

### Bug Fixes

* **release:** unify synthesis grounding (#196) ([b37a82f](https://github.com/misty-step/landmark/commit/b37a82fe66ff036dbc568ecc4855fcc89579dad2))
* **synthesis:** ground release-note sections in real commits, refuse fabrication (#200) ([245c4a5](https://github.com/misty-step/landmark/commit/245c4a545c3eb3be7dd73a0443a88b0f7a54cbf5))
* **ci:** release with an org GitHub App token instead of the expired PAT (#205) ([bb54ab8](https://github.com/misty-step/landmark/commit/bb54ab8f2b96fd7a8c196b0d57d0e09a0997f9dc))
# [1.27.0](https://github.com/misty-step/landmark/compare/v1.26.0...v1.27.0) (2026-07-02)

### Features

* **version:** consolidate landmark run and prepare-self-release onto one engine (#170) ([5ba38ad](https://github.com/misty-step/landmark/commit/5ba38ad603fbe0bd56f4db97138c2e144eff8ce3))
* **schema:** tighten release-context classification/cost, add drift check (#180) ([b335c22](https://github.com/misty-step/landmark/commit/b335c225eb8f7955dd873a2825dc2a888895da2e))
* **release-kit:** adopt native structured output for classification (#188) ([61a39da](https://github.com/misty-step/landmark/commit/61a39dac51a90d5e06d040911c92359ee22ca533))
* **release:** publish landmark to crates.io on landed releases (#190) ([0c92c67](https://github.com/misty-step/landmark/commit/0c92c67a4d4ea861d2b83a94dd031d9e5f306f44))

### Bug Fixes

* **release-kit:** classify from structured commits, not rendered text (#177) ([77e1c89](https://github.com/misty-step/landmark/commit/77e1c894d46a7eb1fa994e9d11e3088b028cb622))
* **release-kit:** populate changed_files in the deterministic context (#178) ([1581d51](https://github.com/misty-step/landmark/commit/1581d5114dfe03e71e5c7d4d38640de832c7896d))
* **release-kit:** scope extract-prs to the release's tag range (#183) ([a3cffca](https://github.com/misty-step/landmark/commit/a3cffca786728c4b79e47e46d6d779457c51c413))
* **release-kit:** stop synthesizing release notes from the wrong changelog section (#184) ([07bfaf3](https://github.com/misty-step/landmark/commit/07bfaf382b1def80f5271256c359ab43a4f1377a))
* **release-kit:** refresh stale model pins, fix classification config-override bug (#187) ([fd2c10c](https://github.com/misty-step/landmark/commit/fd2c10cbccdf84e42d209db71a3c99ea20e7685d))
* **release-kit:** make release-body synthesis idempotent, paginate PR fetch (#191) ([02bc948](https://github.com/misty-step/landmark/commit/02bc9489c2669080aabc1977b21a2a047d9c260e))
# [1.26.0](https://github.com/misty-step/landmark/compare/v1.25.0...v1.26.0) (2026-07-02)

### Features

* **classification:** use structured release signals ([a5ab124](https://github.com/misty-step/landmark/commit/a5ab124d547819d7dba14af5a0ce6a2618272957))
* **classification:** call model classifier before synthesis ([332c6d1](https://github.com/misty-step/landmark/commit/332c6d1c289cafe88d5d6466016575340c5e6b6a))
* **classification:** surface model disagreement in notes ([baff359](https://github.com/misty-step/landmark/commit/baff3599a2b6fd1c6ed79add936cb8bd4563e964))
* **release:** publish per-target binaries, bootstrap action via download (#168) ([6647390](https://github.com/misty-step/landmark/commit/6647390a55b79fc5de3a01551528060a9d7d872b))
# [1.25.0](https://github.com/misty-step/landmark/compare/v1.24.0...v1.25.0) (2026-06-25)

### Features

* **fleet:** deliver backfill-first adoption lane ([bddbfb2](https://github.com/misty-step/landmark/commit/bddbfb2824e783ca94b8924deabb9bd85b3d9e2b))
* **run:** emit release kit artifact graph (#153) ([587ca13](https://github.com/misty-step/landmark/commit/587ca13f0c9f4fa1bf054dfd8dd66aee9bb353b3))

### Bug Fixes

* **fleet:** attach to existing release workflows ([03c65b2](https://github.com/misty-step/landmark/commit/03c65b264a6a6a899783e6df04236b866823a882))
# [1.24.0](https://github.com/misty-step/landmark/compare/v1.23.1...v1.24.0) (2026-06-16)

### Features

* **run:** add provider-neutral release pipeline ([2281fc4](https://github.com/misty-step/landmark/commit/2281fc49da63d6fff0924ea2fdc75ffefa85337d))
* **agent:** publish machine-readable Landfall contracts ([5a6ed75](https://github.com/misty-step/landmark/commit/5a6ed75995015e63cb29bb0a9f34f432bc717e28))
* **adoption:** make first run local preview obvious ([7359b43](https://github.com/misty-step/landmark/commit/7359b43ed19ad6283129b1612942e6e385cb4cd2))
* **fleet:** guard rollout adoption ([c02e41a](https://github.com/misty-step/landmark/commit/c02e41a2224090cc0c33615e6c71a6fc82cdbd5d))
* **synthesis:** emit contextual release intelligence (#141) ([c89e5c3](https://github.com/misty-step/landmark/commit/c89e5c384ee58f915a879d41bb61cc5f3e1bcca6))

### Bug Fixes

* **verification:** prove release side-effect guarantees ([150dc69](https://github.com/misty-step/landmark/commit/150dc696094f85a158c5b479509ae02bbe3ab8a8))
* **fleet:** recognize org-level release secrets ([e9dad85](https://github.com/misty-step/landmark/commit/e9dad85e28719a13b4fef351e2972a6bea361c4f))
# [1.23.1](https://github.com/misty-step/landmark/compare/v1.23.0...v1.23.1) (2026-06-13)

### Bug Fixes

* **action:** tolerate no-release summary artifacts (#131) ([7b1bf02](https://github.com/misty-step/landmark/commit/7b1bf02472d6636f921408322b23a1c3a31bc781))
# [1.23.0](https://github.com/misty-step/landmark/compare/v1.22.0...v1.23.0) (2026-06-13)

### Features

* **fleet:** harden Landmark dogfood adoption (#129) ([52a7ba8](https://github.com/misty-step/landmark/commit/52a7ba866be433483e20e15162db4e637c5c705b))
# [1.22.0](https://github.com/misty-step/landmark/compare/v1.21.0...v1.22.0) (2026-06-13)

### Features

* **backfill:** restore release history migration ([3f0fb5c](https://github.com/misty-step/landmark/commit/3f0fb5cea945a0e223d469bd319849a915cc6769))

# [1.21.0](https://github.com/misty-step/landmark/compare/v1.20.0...v1.21.0) (2026-06-13)

### Features

* **synthesis:** add cost-aware contextual policy ([8a891ef](https://github.com/misty-step/landmark/commit/8a891efc73e58f07221cfd533ef0bb17714a8ab6))
# [1.20.0](https://github.com/misty-step/landmark/compare/v1.19.0...v1.20.0) (2026-06-13)

### Features

* **fleet:** build Landmark adoption planner ([0dbcfeb](https://github.com/misty-step/landmark/commit/0dbcfeb0083e0a173cf65fbca3db3f0a544cf076))
# [1.19.0](https://github.com/misty-step/landmark/compare/v1.18.2...v1.19.0) (2026-06-13)

### Features

* **manifest:** add Landmark product manifest ([c15887e](https://github.com/misty-step/landmark/commit/c15887ea6c7b455b0117b232c3bfd96e7189d741))
# [1.18.2](https://github.com/misty-step/landmark/compare/v1.18.1...v1.18.2) (2026-06-12)

### Bug Fixes

* **release:** keep self-release binary in sync ([1832326](https://github.com/misty-step/landmark/commit/18323261d0f2631e6ff493196240f72e17117b07))
# [1.18.1](https://github.com/misty-step/landmark/compare/v1.18.0...v1.18.1) (2026-06-12)

### Bug Fixes

* **release:** stabilize self-release post-publish gates ([3bb2bdd](https://github.com/misty-step/landmark/commit/3bb2bddf45c3babca63607fb662e7bba497a6bd2))
# [1.18.0](https://github.com/misty-step/landmark/compare/v1.17.2...v1.18.0) (2026-06-12)

### Features

* **runtime:** migrate owned Landmark runtime to Rust (#109) ([3ad9dcb](https://github.com/misty-step/landmark/commit/3ad9dcb25783053a5c7f56c7c2c4c6cf0c2357b8))
* **notes:** add typed release note artifact plane (#110) ([58c814a](https://github.com/misty-step/landmark/commit/58c814a5db4f7f341097d57c75ada11199878eb8))
* **setup:** add adoption analyzer and workflow generator (#111) ([2e30c8d](https://github.com/misty-step/landmark/commit/2e30c8dd7da47aae5f8694c2ebad75534cd35741))
* **release:** add pr-based self-release flow (#112) ([81e9711](https://github.com/misty-step/landmark/commit/81e9711c89fe6e0e1bb683778ddc2299707d846e))

### Bug Fixes

* **healthcheck:** auto-run when synthesis enabled, add OpenRouter-specific 401 message (#99) ([9a1e4c9](https://github.com/misty-step/landmark/commit/9a1e4c9165661e6aa2e6b4a8fe1c645b4fa34879))
* **ci:** avoid duplicate trufflehog fail flag (#106) ([12441ec](https://github.com/misty-step/landmark/commit/12441ec32afb35156e1c00eb62a3c32514919b8d))
* **ci:** allow release candidate metadata checks (#114) ([f645ea0](https://github.com/misty-step/landmark/commit/f645ea0e36eeb544ff24da40600b36f21618b7a2))
## [1.17.2](https://github.com/misty-step/landmark/compare/v1.17.1...v1.17.2) (2026-02-13)


### Bug Fixes

* default synthesis-failure-issue to false ([a83f8b9](https://github.com/misty-step/landmark/commit/a83f8b9511ea6910491f1b201aeb28066a126b79))

## [1.17.1](https://github.com/misty-step/landmark/compare/v1.17.0...v1.17.1) (2026-02-13)


### Bug Fixes

* preflight check for orphaned tag history ([#86](https://github.com/misty-step/landmark/issues/86)) ([#89](https://github.com/misty-step/landmark/issues/89)) ([381572c](https://github.com/misty-step/landmark/commit/381572ccb8ceeb6d97d08be3bce7df344fb51fee))

# [1.17.0](https://github.com/misty-step/landmark/compare/v1.16.1...v1.17.0) (2026-02-12)


### Features

* generic webhook notification on release ([#87](https://github.com/misty-step/landmark/issues/87)) ([b32a2db](https://github.com/misty-step/landmark/commit/b32a2db1e0650d9266b302f661802c354034d290)), closes [#59](https://github.com/misty-step/landmark/issues/59)

## [1.16.1](https://github.com/misty-step/landmark/compare/v1.16.0...v1.16.1) (2026-02-12)


### Bug Fixes

* handle floating tags and deduplicate failure issues ([#84](https://github.com/misty-step/landmark/issues/84)) ([#85](https://github.com/misty-step/landmark/issues/85)) ([faa8c77](https://github.com/misty-step/landmark/commit/faa8c77c908fb03e193f68ad12252cb28e7e6dfa))

# [1.16.0](https://github.com/misty-step/landmark/compare/v1.15.0...v1.16.0) (2026-02-12)


### Features

* post-synthesis output validation with retry ([#83](https://github.com/misty-step/landmark/issues/83)) ([51ea744](https://github.com/misty-step/landmark/commit/51ea744c8e4165146b6b2441b7a2d3538f1f09b6)), closes [#57](https://github.com/misty-step/landmark/issues/57) [#57](https://github.com/misty-step/landmark/issues/57)

# [1.15.0](https://github.com/misty-step/landmark/compare/v1.14.0...v1.15.0) (2026-02-12)


### Features

* highlight breaking changes in notes ([#55](https://github.com/misty-step/landmark/issues/55)) ([#80](https://github.com/misty-step/landmark/issues/80)) ([af81075](https://github.com/misty-step/landmark/commit/af810757a8c11139819d2a4013bab41799b46642))

# [1.14.0](https://github.com/misty-step/landmark/compare/v1.13.0...v1.14.0) (2026-02-12)


### Features

* **synthesis:** release significance detection ([#79](https://github.com/misty-step/landmark/issues/79)) ([a493f0b](https://github.com/misty-step/landmark/commit/a493f0b438ca74f2056eab97ca7af68724509287)), closes [#54](https://github.com/misty-step/landmark/issues/54)

# [1.13.0](https://github.com/misty-step/landmark/compare/v1.12.0...v1.13.0) (2026-02-12)


### Features

* **synthesis:** add PR-based changelog source selection ([#77](https://github.com/misty-step/landmark/issues/77)) ([9109621](https://github.com/misty-step/landmark/commit/9109621282e4c53a8d6dbaf2776cd917fb9f7b01))

# [1.12.0](https://github.com/misty-step/landmark/compare/v1.11.0...v1.12.0) (2026-02-12)


### Features

* add audience-specific synthesis prompt variants ([#76](https://github.com/misty-step/landmark/issues/76)) ([0b5ccc7](https://github.com/misty-step/landmark/commit/0b5ccc70dab76e6b384b838e042d7d83d1abf49f))

# [1.11.0](https://github.com/misty-step/landmark/compare/v1.10.0...v1.11.0) (2026-02-12)


### Features

* support targeted release-note resynthesis backfills ([#74](https://github.com/misty-step/landmark/issues/74)) ([eb3b553](https://github.com/misty-step/landmark/commit/eb3b5537df4d3695745e9180b8f6b550e7e2119a))

# [1.10.0](https://github.com/misty-step/landmark/compare/v1.9.0...v1.10.0) (2026-02-11)


### Features

* auto-close stale synthesis failure issues on success ([#73](https://github.com/misty-step/landmark/issues/73)) ([b276399](https://github.com/misty-step/landmark/commit/b2763993c9b959f9dc85fb7a8b502a150f404223)), closes [#45](https://github.com/misty-step/landmark/issues/45)

# [1.9.0](https://github.com/misty-step/landmark/compare/v1.8.0...v1.9.0) (2026-02-11)


### Bug Fixes

* don't create failure issues when API key is unconfigured ([#67](https://github.com/misty-step/landmark/issues/67)) ([30adeb0](https://github.com/misty-step/landmark/commit/30adeb07c79a2d91dabc83460c109cfda08b4de2)), closes [#46](https://github.com/misty-step/landmark/issues/46)
* surface actionable diagnosis for 401/403 LLM API errors ([#68](https://github.com/misty-step/landmark/issues/68)) ([c893b9c](https://github.com/misty-step/landmark/commit/c893b9c08302ae9588faaea9f012011c58dee7b8)), closes [#47](https://github.com/misty-step/landmark/issues/47)


### Features

* support custom synthesis prompt templates ([#66](https://github.com/misty-step/landmark/issues/66)) ([8d022d1](https://github.com/misty-step/landmark/commit/8d022d13c8b3d3ffe9fe598ffc5ba3f94fe380a5)), closes [#15](https://github.com/misty-step/landmark/issues/15)

# [1.8.0](https://github.com/misty-step/landmark/compare/v1.7.0...v1.8.0) (2026-02-11)


### Features

* add proactive API key health check ([#69](https://github.com/misty-step/landmark/issues/69)) ([7d943cb](https://github.com/misty-step/landmark/commit/7d943cbc7170da281ba4e304ab6afcd8b3e06c9b)), closes [#49](https://github.com/misty-step/landmark/issues/49)
* add synthesis-only mode to decouple from semantic-release ([#70](https://github.com/misty-step/landmark/issues/70)) ([3fa37e1](https://github.com/misty-step/landmark/commit/3fa37e109409bb8684760c8cb81dbdd21dff8fc2)), closes [#50](https://github.com/misty-step/landmark/issues/50)
* rewrite synthesis system message and prompt template ([#71](https://github.com/misty-step/landmark/issues/71)) ([04822aa](https://github.com/misty-step/landmark/commit/04822aa8b299264b1a4d620843e68f5aeda83c27)), closes [#53](https://github.com/misty-step/landmark/issues/53)

# [1.7.0](https://github.com/misty-step/landmark/compare/v1.6.1...v1.7.0) (2026-02-11)


### Features

* support consuming repo .releaserc override ([#65](https://github.com/misty-step/landmark/issues/65)) ([a173a38](https://github.com/misty-step/landmark/commit/a173a38142e102624e76290bfabeb651391de2a1)), closes [#14](https://github.com/misty-step/landmark/issues/14)

## [1.6.1](https://github.com/misty-step/landmark/compare/v1.6.0...v1.6.1) (2026-02-11)


### Bug Fixes

* make action dependency installs deterministic ([#42](https://github.com/misty-step/landmark/issues/42)) ([31d06ca](https://github.com/misty-step/landmark/commit/31d06ca30e471771e56d11ae3272c185250f8467)), closes [#35](https://github.com/misty-step/landmark/issues/35)

# [1.6.0](https://github.com/misty-step/landmark/compare/v1.5.0...v1.6.0) (2026-02-10)


### Features

* add floating major version tag support for GitHub Actions repos ([#28](https://github.com/misty-step/landmark/issues/28)) ([#40](https://github.com/misty-step/landmark/issues/40)) ([7cbb7a0](https://github.com/misty-step/landmark/commit/7cbb7a0e73f6057553cbde7a312407070e468517))

# [1.5.0](https://github.com/misty-step/landmark/compare/v1.4.0...v1.5.0) (2026-02-10)


### Features

* backfill CLI for retroactive release note synthesis ([#39](https://github.com/misty-step/landmark/issues/39)) ([8b57870](https://github.com/misty-step/landmark/commit/8b57870dfdf3fe85147be24758aa59e224601e11)), closes [#13](https://github.com/misty-step/landmark/issues/13) [#13](https://github.com/misty-step/landmark/issues/13)

# [1.4.0](https://github.com/misty-step/landmark/compare/v1.3.4...v1.4.0) (2026-02-10)


### Features

* add plaintext/html artifact outputs ([#12](https://github.com/misty-step/landmark/issues/12)) ([#38](https://github.com/misty-step/landmark/issues/38)) ([dcfb14f](https://github.com/misty-step/landmark/commit/dcfb14f21924a7a5686c2d27338379dd29c45bb3))

## [1.3.4](https://github.com/misty-step/landmark/compare/v1.3.3...v1.3.4) (2026-02-10)


### Bug Fixes

* **ci:** correct test directory paths and add ruff config ([#33](https://github.com/misty-step/landmark/issues/33)) ([0f795e0](https://github.com/misty-step/landmark/commit/0f795e0ca758e5581379a364c46d3495172454f3)), closes [#11](https://github.com/misty-step/landmark/issues/11)

## [1.3.3](https://github.com/misty-step/landmark/compare/v1.3.2...v1.3.3) (2026-02-10)


### Bug Fixes

* **ci:** add dedicated workflow to sync v1 tag ([#32](https://github.com/misty-step/landmark/issues/32)) ([04c91c5](https://github.com/misty-step/landmark/commit/04c91c55c9740f05ceefa7f063e469b2bcf1211d)), closes [#16](https://github.com/misty-step/landmark/issues/16)

## [1.3.2](https://github.com/misty-step/landmark/compare/v1.3.1...v1.3.2) (2026-02-10)


### Bug Fixes

* fall back to GitHub release body when CHANGELOG.md is missing ([#31](https://github.com/misty-step/landmark/issues/31)) ([1384cf5](https://github.com/misty-step/landmark/commit/1384cf50a3d237ab4651ff2e662d4ca8734e33af)), closes [misty-step/cerberus#82](https://github.com/misty-step/cerberus/issues/82)

## [1.3.1](https://github.com/misty-step/landmark/compare/v1.3.0...v1.3.1) (2026-02-09)


### Bug Fixes

* keep changelog semantic-release only ([#27](https://github.com/misty-step/landmark/issues/27)) ([5f9a235](https://github.com/misty-step/landmark/commit/5f9a2352c379b82687fa997d214a50cdfcd2ee94))

# [1.3.0](https://github.com/misty-step/landmark/compare/v1.2.0...v1.3.0) (2026-02-09)


### Features

* generate portable release notes artifacts ([#26](https://github.com/misty-step/landmark/issues/26)) ([d4ca901](https://github.com/misty-step/landmark/commit/d4ca90199c4b022407ee8ba2705d2a385100356f)), closes [#7](https://github.com/misty-step/landmark/issues/7)

# [1.2.0](https://github.com/misty-step/landmark/compare/v1.1.5...v1.2.0) (2026-02-09)


### Features

* alert and signal synthesis failures ([#25](https://github.com/misty-step/landmark/issues/25)) ([8398ca0](https://github.com/misty-step/landmark/commit/8398ca066c51c45a025adff1e536c0bbdf2d5202))

## [1.1.5](https://github.com/misty-step/landmark/compare/v1.1.4...v1.1.5) (2026-02-09)


### Bug Fixes

* remove unused @semantic-release/npm dependency ([#24](https://github.com/misty-step/landmark/issues/24)) ([a353646](https://github.com/misty-step/landmark/commit/a353646e21c3381e440536e1c3ab3435dbeb3959)), closes [#5](https://github.com/misty-step/landmark/issues/5)

## [1.1.4](https://github.com/misty-step/landmark/compare/v1.1.3...v1.1.4) (2026-02-09)


### Bug Fixes

* harden self-release notes pipeline ([#23](https://github.com/misty-step/landmark/issues/23)) ([0a030b4](https://github.com/misty-step/landmark/commit/0a030b4c21e88daf5b5d68fd75cae2b83ce9938f))

## [1.1.3](https://github.com/misty-step/landmark/compare/v1.1.2...v1.1.3) (2026-02-08)


### Bug Fixes

* remove dead backward-compat code and warn on insecure API URLs ([#20](https://github.com/misty-step/landmark/issues/20)) ([0df6c21](https://github.com/misty-step/landmark/commit/0df6c21d601c60e71c25a86184b4ac67499535d4)), closes [#3](https://github.com/misty-step/landmark/issues/3) [#4](https://github.com/misty-step/landmark/issues/4)

## [1.1.2](https://github.com/misty-step/landmark/compare/v1.1.1...v1.1.2) (2026-02-08)


### Bug Fixes

* provider-agnostic LLM inputs for release synthesis ([#19](https://github.com/misty-step/landmark/issues/19)) ([451db2a](https://github.com/misty-step/landmark/commit/451db2a01256030bedd0039396af86e6f6a5ac03)), closes [#4](https://github.com/misty-step/landmark/issues/4)

## [1.1.1](https://github.com/misty-step/landmark/compare/v1.1.0...v1.1.1) (2026-02-08)


### Bug Fixes

* remove npm plugin and package.json references for non-Node project support ([#17](https://github.com/misty-step/landmark/issues/17)) ([c5e9dc0](https://github.com/misty-step/landmark/commit/c5e9dc0257622fff7914ac2db49732d764c39296)), closes [misty-step/vox#178](https://github.com/misty-step/vox/issues/178)

# [1.1.0](https://github.com/misty-step/landmark/compare/v1.0.0...v1.1.0) (2026-02-08)


### Bug Fixes

* harden release workflow template (concurrency, timeout, docs) ([#2](https://github.com/misty-step/landmark/issues/2)) ([228f57f](https://github.com/misty-step/landmark/commit/228f57f67cb93db2d7b4d9ebfed6a4a485f330e3))


### Features

* integrate Landmark release pipeline ([#1](https://github.com/misty-step/landmark/issues/1)) ([2d36967](https://github.com/misty-step/landmark/commit/2d36967ac612d228fa03905bc664cc4af74cd1d1))
