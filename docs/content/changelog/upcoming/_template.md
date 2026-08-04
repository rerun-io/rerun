---
title: Changeset entry (template)
hidden: true
type: feature # highlight | breaking | feature
---

<!--
══════════════════════════════════════════════════════════════════════════════
 PER-PR CHANGESET ENTRY TEMPLATE

 If your PR is labeled `include in changelog`, copy this file to
 `upcoming/<short-slug>.md` (e.g. `upcoming/log-tick-default-off.md`) and write
 your entry below. One file per PR — this avoids the merge conflicts you'd get
 from everyone editing a single shared changeset.

 Frontmatter:
   title  — your entry's heading (the docs build requires every page under
            docs/content/ to have one).
   hidden — keep `true`; it keeps the in-flight entry out of the navigation.
   type   — which section the entry belongs in. One of:
              highlight  — a flagship change worth selling at the top of the notes
              breaking   — a breaking change; MUST include a migration guide
              feature    — a user-facing feature; SHOULD link docs and/or an example

 At release time, an agent merges every file in this folder into the release's
 `changeset-0-XX.md`, grouping by `type`, then empties the folder.

 For each entry, consider:
   * Migration guide   — required for any breaking change. Show before/after.
   * Screenshot or GIF — required for any visual feature. (GIFs render better
                         than mp4 on GitHub; upload to a PR to host the image.)
   * Docs link         — required for any new user-facing feature.
   * Example link      — required for new SDK functionality.

 Write any relative doc links as if from `changelog/` (e.g.
 `../reference/migration/...`), since that is where this entry ends up once
 merged. This folder is excluded from link checking for that reason.

 A placeholder like `TODO(name): add docs link` is fine while iterating, but it
 must be resolved before the release ships — the release is BLOCKED on any
 unresolved `TODO(name)`.

 Delete this comment block in your copy.
══════════════════════════════════════════════════════════════════════════════
-->

### My new feature

Short description of what changed and why a user should care.

Docs: TODO(name): add docs link
Example: TODO(name): add example link
