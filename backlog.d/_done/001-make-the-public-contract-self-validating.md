# Make the public contract self-validating

Priority: P1 · Status: done · Estimate: L

## Goal
Keep Landmark's README, action metadata, examples, and project notes generated or checked from one source of truth so consumers never follow stale release-action contracts.

## Oracle
- [x] A local command fails when README input defaults diverge from `action.yml`.
- [x] The command checks examples and project-facing docs for deprecated or removed inputs.
- [x] The README inputs table is regenerated or mechanically validated from `action.yml`.
- [x] `python -m pytest -q tests/` covers a known drift fixture such as `synthesis-failure-issue`.

## Children
1. Add an action-contract parser that extracts inputs, defaults, required flags, and deprecation metadata from `action.yml`.
2. Replace the hand-maintained README input table with generated content or a checked snapshot.
3. Check `examples/*.yml`, `.github/workflows/*.yml`, `project.md`, and `CLAUDE.md` for removed/deprecated inputs and stale active-focus references.
4. Decide the `synthesis-strict` removal path and make the docs, action metadata, and dogfood workflow agree.
5. Add the contract check to CI before adoption work resumes.

## Notes
- Evidence: `README.md` documented `synthesis-failure-issue` default as `true` while `action.yml` sets it to `false`; this groom fixed the visible README row, but not the root cause.
- Evidence: `project.md` still named closed issues `#88` and `#90` as active focus before this groom.
- Why: product/adoption and harness-health perspectives both found the same root cause: public contract drift is currently detectable only by manual reading.

## Delivery
- Added `scripts/check-action-contract.py` as the local and CI contract validation command.
- The checker validates README input names, order, required flags, and defaults against `action.yml`.
- The checker scans README/CLAUDE/project docs, examples, and release workflows for unknown or deprecated Landmark inputs in usage blocks.
- Verification: `python scripts/check-action-contract.py`; `python -m pytest -q tests/test_check_action_contract.py`.
