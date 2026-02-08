# [1.1.0](https://github.com/misty-step/landfall/compare/v1.0.0...v1.1.0) (2026-02-08)


### Bug Fixes

* harden release workflow template (concurrency, timeout, docs) ([#2](https://github.com/misty-step/landfall/issues/2)) ([228f57f](https://github.com/misty-step/landfall/commit/228f57f67cb93db2d7b4d9ebfed6a4a485f330e3))


### Features

* integrate Landfall release pipeline ([#1](https://github.com/misty-step/landfall/issues/1)) ([2d36967](https://github.com/misty-step/landfall/commit/2d36967ac612d228fa03905bc664cc4af74cd1d1))

# Changelog

All notable changes to Landfall are documented in this file.

The format is based on Keep a Changelog and uses Semantic Versioning.

## [Unreleased]

### Added
- Unit test coverage for synthesis and release update scripts.
- CI workflow for linting, tests, and `action.yml` schema validation.
- Example consumer release workflow template under `examples/release.yml`.

### Changed
- Hardened HTTP handling in synthesis and release update scripts with retries.
- Added structured logging and CLI input validation for Python scripts.
- Improved synthesis prompt guidance for concise, user-friendly release notes.
- Made synthesis and release-note update failures non-blocking in the composite action.
