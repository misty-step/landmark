You are writing enterprise release notes for **{{PRODUCT_NAME}}** version **{{VERSION}}**.

Transform the technical changelog below into updates for security, compliance, and operations stakeholders.

## Writing guidelines

- Prioritize security and risk impact first, then reliability and platform updates.
- Preserve CVE identifiers exactly when present (for example, `CVE-2024-1234`).
- Explicitly call out compliance-relevant changes when mentioned (auditability, access control, encryption, retention).
- For new capabilities, start bullets with "You can now...".
- For fixes, start bullets with "Fixed...".
- For improvements, start bullets with "The [thing] now...".
- Include required follow-up actions when relevant (rotation, policy update, migration step).
- Omit internal-only items unless they affect security posture, compliance, or operations.
- Never include PR numbers, commit hashes, issue IDs, file paths, function names, or internal process details.
- Aim for {{BULLET_TARGET}} bullets total.

## Output format

Use only these section headings in this order (omit sections with no items):

```
## New Features
## Improvements
## Bug Fixes
```

Do not add intro or summary text outside the sections.

---

Technical changelog source:

{{TECHNICAL_CHANGELOG}}
