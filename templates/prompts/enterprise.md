You are writing enterprise release notes for **{{PRODUCT_NAME}}** version **{{VERSION}}**.

Transform the technical changelog below into updates for security, compliance, and operations stakeholders.

{{PRODUCT_CONTEXT}}

{{VOICE_GUIDE}}

{{BREAKING_CHANGES_SECTION}}

## Writing guidelines

- Breaking changes: If breaking changes are provided above, write them first under `## Breaking Changes`. 2-3 sentences each: what changed, risk, required migration steps. Do not repeat them in other sections.
- Prioritize security and risk impact first, then reliability and platform updates.
- Preserve CVE identifiers exactly when present (for example, `CVE-2024-1234`).
- Explicitly call out compliance-relevant changes when mentioned (auditability, access control, encryption, retention).
- For new capabilities, lead with the operational or compliance-relevant benefit.
- For fixes, state plainly what was broken and that it's fixed.
- For improvements, show what got better.
- Vary how bullets open. Do not start every feature bullet with "You can now," every fix
  with "Fixed," or every improvement with "The [thing] now" — that repetition reads as
  templated. Let each bullet's specific content dictate its own natural phrasing.
- Include required follow-up actions when relevant (rotation, policy update, migration step).
- Omit internal-only items unless they affect security posture, compliance, or operations.
- Never include PR numbers, commit hashes, issue IDs, file paths, function names, or internal process details.
- Aim for {{BULLET_TARGET}} bullets total.

## Output format

Use only these section headings in this order (omit sections with no items):

```
## Breaking Changes
## New Features
## Improvements
## Bug Fixes
```

Do not add intro or summary text outside the sections.

---

Technical changelog source:

Grounding rule: treat the release commit list in the source below as authoritative. If supplemental changelog, release-body, or PR text conflicts with those commits or the diff stats, ignore the supplemental text.

{{TECHNICAL_CHANGELOG}}
