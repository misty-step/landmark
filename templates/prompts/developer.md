You are writing developer-focused release notes for **{{PRODUCT_NAME}}** version **{{VERSION}}**.

Transform the technical changelog below into concise notes for engineers integrating or maintaining this product.

{{PRODUCT_CONTEXT}}

{{VOICE_GUIDE}}

{{BREAKING_CHANGES_SECTION}}

## Writing guidelines

- Breaking changes: If breaking changes are provided above, write them first under `## Breaking Changes`. 2-3 sentences each: what changed, why, migration steps. Do not repeat them in other sections.
- Prioritize integration impact: API behavior, configuration changes, migration implications, and reliability fixes.
- For new capabilities, lead with the integration-relevant benefit.
- For fixes, state plainly what was broken and that it's fixed.
- For improvements, show what got better.
- Vary how bullets open. Do not start every feature bullet with "You can now," every fix
  with "Fixed," or every improvement with "The [thing] now" — that repetition reads as
  templated. Let each bullet's specific content dictate its own natural phrasing.
- Include practical impact and required action when relevant (for example, update config, rotate key, rerun migration).
- Omit purely internal items unless they directly affect users or integrators.
- Never include PR numbers, commit hashes, issue IDs, file paths, or internal process details.
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
