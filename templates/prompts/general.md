You are writing release notes for **{{PRODUCT_NAME}}** version **{{VERSION}}**.

Transform the technical changelog below into user-facing release notes.

{{PRODUCT_CONTEXT}}

{{VOICE_GUIDE}}

{{BREAKING_CHANGES_SECTION}}

## Writing guidelines

- **Breaking changes:** If breaking changes are provided above, write them first under `## Breaking Changes`. 2-3 sentences each: what changed, why, migration steps. Do not repeat them in other sections.
- **Features:** Frame new capabilities from the user's perspective and lead with the benefit.
- **Bug fixes:** State plainly what was broken and that it's fixed.
- **Improvements:** Show what got better and by how much, when the changelog says.
- Vary how bullets open. Do not start every feature bullet with "You can now," every fix
  with "Fixed," or every improvement with "The [thing] now" — that repetition reads as
  templated. Let each bullet's specific content dictate its own natural phrasing.
- Each non-breaking bullet should be one concise sentence explaining what changed and why it matters.
- Omit internal-only items (CI, tooling, refactors, dependency bumps) unless they have user-visible impact.
- Never include PR numbers, commit hashes, issue IDs, file paths, function names, or internal process details.
- Aim for {{BULLET_TARGET}} bullets total. More for feature-rich releases, fewer for patches.

## Output format

Use only these section headings in this order (omit sections with no items):

```
## Breaking Changes
## New Features
## Improvements
## Bug Fixes
```

Do not add any intro, outro, summary, or sign-off text. Start directly with the first `##` heading.

---

Technical changelog source:

{{TECHNICAL_CHANGELOG}}
