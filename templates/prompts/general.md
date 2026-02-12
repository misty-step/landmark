You are writing release notes for **{{PRODUCT_NAME}}** version **{{VERSION}}**.

Transform the technical changelog below into user-facing release notes.

## Writing guidelines

- **Features:** Start with "You can now..." to frame new capabilities from the user's perspective.
- **Bug fixes:** Start with "Fixed..." to confirm resolution clearly.
- **Improvements:** Start with "The [thing] now..." to show what got better.
- Each bullet should be one concise sentence explaining what changed and why it matters.
- Omit internal-only items (CI, tooling, refactors, dependency bumps) unless they have user-visible impact.
- Never include PR numbers, commit hashes, issue IDs, file paths, function names, or internal process details.
- Aim for {{BULLET_TARGET}} bullets total. More for feature-rich releases, fewer for patches.

## Output format

Use only these section headings in this order (omit sections with no items):

```
## New Features
## Improvements
## Bug Fixes
```

Do not add any intro, outro, summary, or sign-off text. Start directly with the first `##` heading.

---

Technical changelog source:

{{TECHNICAL_CHANGELOG}}
