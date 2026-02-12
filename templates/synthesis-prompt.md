You are writing release notes for **{{PRODUCT_NAME}}** version **{{VERSION}}**.

Transform the technical changelog below into user-facing release notes.

{{BREAKING_CHANGES_SECTION}}

## Writing guidelines

- **Breaking changes:** If breaking changes are provided above, write them first under `## Breaking Changes`. 2-3 sentences each: what changed, why, migration steps. Do not repeat them in other sections.
- **Features:** Start with "You can now..." to frame new capabilities from the user's perspective.
- **Bug fixes:** Start with "Fixed..." to confirm resolution clearly.
- **Improvements:** Start with "The [thing] now..." to show what got better.
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

## Examples

### Example 1: Feature release

Technical changelog:
### Features
- add one-click workspace import command
- support custom theme colors in settings
### Bug Fixes
- retry webhook processing when signatures expire
### Chores
- bump CI cache key

Expected release notes:
## New Features
- You can now import workspace configuration in one click, reducing initial setup time.
- You can now customize theme colors from the settings page.

## Bug Fixes
- Fixed webhook deliveries failing silently when signatures expired.

### Example 2: Patch release

Technical changelog:
### Bug Fixes
- fix dashboard crash when saving empty profile fields
### Refactor
- split parser module into smaller files

Expected release notes:
## Bug Fixes
- Fixed a dashboard crash that occurred when saving a profile with empty fields.

### Example 3: Breaking change

Technical changelog:
### BREAKING CHANGES
- remove deprecated /v1/auth endpoint
### Features
- add OAuth 2.0 PKCE authentication flow

Expected release notes:
## Breaking Changes
- The deprecated `/v1/auth` endpoint was removed to simplify the authentication surface area. If you were using it, migrate to the new OAuth 2.0 PKCE flow before upgrading.

## New Features
- You can now authenticate using OAuth 2.0 with PKCE for a more secure sign-in flow.

---

Technical changelog source:

{{TECHNICAL_CHANGELOG}}
