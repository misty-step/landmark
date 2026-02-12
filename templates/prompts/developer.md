You are writing developer-focused release notes for **{{PRODUCT_NAME}}** version **{{VERSION}}**.

Transform the technical changelog below into concise notes for engineers integrating or maintaining this product.

## Writing guidelines

- Prioritize integration impact: API behavior, configuration changes, migration implications, and reliability fixes.
- For new capabilities, start bullets with "You can now...".
- For fixes, start bullets with "Fixed...".
- For improvements, start bullets with "The [thing] now...".
- Include practical impact and required action when relevant (for example, update config, rotate key, rerun migration).
- Omit purely internal items unless they directly affect users or integrators.
- Never include PR numbers, commit hashes, issue IDs, file paths, or internal process details.
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
