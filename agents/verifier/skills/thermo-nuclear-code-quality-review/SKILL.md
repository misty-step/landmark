---
name: thermo-nuclear-code-quality-review
description: Perform a strict but calibrated maintainability review of one exact Revision. Use with thermo-nuclear-review during every Iron Forest Verifier review.
license: MIT
metadata:
  adapted-from: https://github.com/cursor/plugins/tree/main/cursor-team-kit/skills/thermo-nuclear-code-quality-review
---

# Deep code-quality review

Search for a simpler design that removes concepts, branches, layers, or special cases. Prefer direct and boring code.

Check whether the Revision:

- Uses the existing architecture and canonical helpers.
- Puts behavior in the layer that owns it.
- Adds scattered conditions, modes, flags, or silent fallbacks.
- Adds wrappers or abstractions that do not reduce complexity.
- Hides an invariant behind casts, optional values, or loose data shapes.
- Duplicates logic or creates a second convention.
- Mixes orchestration with business logic.
- Makes related state changes less atomic.
- Grows a file or function until its purpose is hard to scan.
- Leaves obsolete code, comments, aliases, or temporary scaffolding.

Propose the structural move when you report a problem. Prefer deletion and reframing over moving the same complexity.

Block approval only for a material maintainability regression or a concrete engineering risk. Mark a cleaner alternative as optional when the current design is clear, consistent, and safe. Do not block on taste, formatting, or a different personal implementation.

Report a few high-confidence findings. Do not produce a list of cosmetic comments.
