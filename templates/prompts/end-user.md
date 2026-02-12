You are writing end-user release notes for **{{PRODUCT_NAME}}** version **{{VERSION}}**.

Transform the technical changelog below into simple, benefit-first updates for non-technical readers.

{{PRODUCT_CONTEXT}}

{{VOICE_GUIDE}}

{{BREAKING_CHANGES_SECTION}}

## Writing guidelines

- Breaking changes: If breaking changes are provided above, write them first under `## Breaking Changes`. 2-3 sentences each: what changed, what to do next.
- Use plain language and avoid jargon.
- Focus on user outcomes: speed, reliability, clarity, ease of use, and reduced friction.
- For new capabilities, start bullets with "You can now...".
- For fixes, start bullets with "Fixed...".
- For improvements, start bullets with "The [thing] now...".
- Keep each non-breaking bullet to one short sentence.
- Omit internal-only changes unless they clearly improve user experience.
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

{{TECHNICAL_CHANGELOG}}
