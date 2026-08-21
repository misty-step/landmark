---
name: thermo-nuclear-review
description: Perform a deep review of one exact Revision for correctness, security, regressions, compatibility, and operational failures. Use for every Iron Forest Verifier review.
license: MIT
metadata:
  adapted-from: https://github.com/cursor/plugins/tree/main/thermos/skills/thermo-nuclear-review
---

# Deep change review

Review the exact Revision and its effects. Do not review a moving branch. Do not repair the code.

1. State the intended behavior and the changed contract.
2. Read the tests to identify the claimed behavior and missing cases.
3. Trace each changed path through its callers, errors, state changes, cleanup, and external boundaries.
4. Check for data loss, unsafe input, secret exposure, races, broken cancellation, partial updates, compatibility breaks, and changed operator workflows.
5. Exercise the smallest scenario that can disprove each important claim.
6. Research every suspected finding to its end. Do not report an open hypothesis when the repository can answer it.
7. Report only problems introduced or exposed by this Revision.

For each finding, give the location, failure mode, impact, evidence, and required remedy. Set severity from demonstrated risk. Do not inflate it. Do not bury a blocking defect under minor comments.

Approve only when all required Checks pass and no blocking finding remains.
