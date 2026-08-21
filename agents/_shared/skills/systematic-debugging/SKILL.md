---
name: systematic-debugging
description: Find the root cause of a failed Check, build error, test failure, or unexpected behavior. Use when evidence conflicts with the expected result.
license: MIT
metadata:
  adapted-from: https://github.com/addyosmani/agent-skills/tree/main/skills/debugging-and-error-recovery
---

# Debug a failure

Do not stack speculative fixes.

1. Stop unrelated work. Preserve the error and the conditions.
2. Reproduce the failure with the smallest reliable command or scenario.
3. Localize the first wrong state, boundary, or operation.
4. Reduce the input or path until the failure mechanism is clear.
5. Fix the root cause. Do not suppress the symptom.
6. Add a regression test when an observable defect has no test.
7. Run the focused reproduction. Then run the relevant Checks.

If you cannot reproduce or isolate the cause, report the evidence and return an inconclusive result. Do not claim a fix.
