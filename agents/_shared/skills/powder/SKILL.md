---
name: powder
description: >
  Powder is the exclusive-work ledger. Use when listing takeable jobs
  for this repository, taking a job, asking the operator, or completing
  work with proof after an approve Gate.
---

# Powder

Powder stores jobs. Take one. Finish it. Write proof.

## Origin

Origin is `POWDER_URL`, else `POWDER_API_BASE_URL`. Identity is
`POWDER_AGENT`. `--agent` wins. JSON on stdout. Errors are JSON on
stderr with `code`.

If `POWDER_AGENT` is unset, do not call Powder. GitHub Issues remain
the Tracker.

## Factory loop

1. `powder list --mine "$POWDER_AGENT" --repo <forest.yaml repo>`
   Continue a held job for this repository that has no
   `forest/<id>/*` branch.
2. `powder list --takeable --repo <forest.yaml repo>`
3. `powder show <id>`
   The spec is the work. Empty spec is not takeable.
4. `powder take <id>`
   Do this before creating a branch. `already_holding` means finish,
   ask, or release first.
5. Publish with schema v2 and branch `forest/<id>/<slug>`. Every Subject uses that shape, including GitHub Issue numbers.
6. Do not `powder done` from Builder. Verifier calls
   `powder done <id> --proof <revision>` after a successful approve.

One live lease per agent. Use one `POWDER_AGENT` per Kernel.

## Verbs

```
powder list --takeable --repo REPO
powder list --mine AGENT --repo REPO
powder show ID
powder take ID
powder release ID
powder ask ID --question '...'
powder done ID --proof PROOF
```
