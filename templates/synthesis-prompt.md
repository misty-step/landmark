You are writing release notes for **{{PRODUCT_NAME}}** version **{{VERSION}}**.

Transform the technical changelog below into user-facing release notes.

{{PRODUCT_CONTEXT}}

{{VOICE_GUIDE}}

{{BREAKING_CHANGES_SECTION}}

Grounding rule: treat the release commit list in the source below as authoritative. If supplemental changelog, release-body, or PR text conflicts with those commits or the diff stats, ignore the supplemental text.

## Writing guidelines

- **Breaking changes:** If breaking changes are provided above, write them first under `## Breaking Changes`. 2-3 sentences each: what changed, why, migration steps. Do not repeat them in other sections.
- **Features:** Frame new capabilities from the user's perspective and lead with the benefit.
- **Bug fixes:** State plainly what was broken and that it's fixed.
- **Improvements:** Show what got better and by how much, when the changelog says.
- Vary how bullets open. Do not start every feature bullet with "You can now," every fix
  with "Fixed," or every improvement with "The [thing] now" — that repetition reads as
  templated. Let each bullet's specific content dictate its own natural phrasing (see the
  examples below for the range of openings expected).
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
- Import workspace configuration in one click, cutting initial setup time.
- Theme colors are now customizable from the settings page.

## Bug Fixes
- Webhook deliveries no longer fail silently when signatures expire; they retry instead.

### Example 2: Patch release

Technical changelog:
### Bug Fixes
- fix dashboard crash when saving empty profile fields
### Refactor
- split parser module into smaller files

Expected release notes:
## Bug Fixes
- Saving a profile with empty fields no longer crashes the dashboard.

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
- OAuth 2.0 with PKCE is now supported for a more secure sign-in flow.

---

Technical changelog source:

{{TECHNICAL_CHANGELOG}}
