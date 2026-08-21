---
name: verify-claim
description: Verify a behavior, bug-fix, performance, or compatibility claim with current local evidence. Use before you state that an important claim is true.
license: MIT
metadata:
  adapted-from: https://github.com/cursor/plugins/tree/main/cursor-team-kit/skills/verify-this
---

# Verify a claim

Verification is not a summary. It must test a claim that can be false.

1. State the condition and expected result.
2. Select the smallest surface that can disprove the claim.
3. Capture the old result when the claim needs a before-and-after comparison.
4. Capture the new result with the same input and environment.
5. Compare the direct evidence. Use output, responses, measurements, or visible behavior.
6. Report `VERIFIED`, `NOT VERIFIED`, or `INCONCLUSIVE`.

Name the command or scenario. Show the important result. State any limit or confounding condition. Do not turn a passing build into proof of user behavior.
