---
title: "0.XX"
order: 0
hidden: true
---

<!--
══════════════════════════════════════════════════════════════════════════════
 RELEASE CHANGESET TEMPLATE — the curated part of the release notes.

 This file is NOT edited per PR. Instead, it is ASSEMBLED AT RELEASE TIME by an
 agent that merges every entry in `upcoming/` into the sections below, grouped by
 each entry's `type:` hint (`highlight` / `breaking` / `feature`). See
 `upcoming/_template.md` for the per-PR entry format.

 When a release is assembled: copy this file to `changeset-0-XX.md`, set `title`
 to the version, and pick an `order` one lower than the previous release (lower
 order = newer = listed first). Then assemble `upcoming/` into it and empty the
 folder.

 Do this at assembly time, not ahead of it: `scripts/ci/check_changelog_redirect.py`
 requires the newest `changeset-0-xx.md` to be the one `changelog.md` redirects to,
 so a changeset for an unreleased version would fail CI.

 Bug fixes (`🪳 bug`) and performance improvements (`📉 performance`) are auto-collected
 into CHANGELOG.md from PR titles and do NOT get an entry here.

 The release is BLOCKED until every `TODO(name)` below is resolved.
══════════════════════════════════════════════════════════════════════════════
-->

## Highlights

<!-- A few sentences selling the release. Feature leads can fill in their item. -->

TODO(release_manager): write the highlights

## New features

<!--
One `### <heading>` per user-facing feature. Each should link to docs and/or an
example, and include a screenshot/GIF if it has any visual component.
-->

<!--
### My new feature

Short description.

Docs: TODO(name): add docs link
Example: TODO(name): add example link
-->

## Breaking changes

<!--
Kept last on purpose: the changelog leads with what's new for most readers, and
the detailed migration guides — only relevant to developers upgrading — sit at
the bottom.

One `### <heading>` per breaking change, each with a migration guide:
what changed, why it is OK, and the before/after the user needs to apply.
If there are none, write "None.".
-->

None.

---

Looking for an older release? See the [migration guides for 0.33 and earlier](../reference/migration.md).
