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
## New Features
- You can now authenticate using OAuth 2.0 with PKCE, replacing the deprecated v1 auth flow. If you were using the previous `/v1/auth` endpoint, switch to the new OAuth flow — see the migration guide for details.

---

Technical changelog source:

{{TECHNICAL_CHANGELOG}}
