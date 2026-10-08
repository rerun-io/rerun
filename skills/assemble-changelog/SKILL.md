---
name: assemble-changelog
description: >
  Assemble the per-PR changeset entries in docs/content/changelog/upcoming/
  into a release's changeset-0-XX.md and generate the detailed CHANGELOG.md sections.
  Use at release time when the user wants to build or finalize the changelog or
  changeset for a Rerun release, or merge the upcoming/ entries. Invoked via
  /assemble-changelog or "assemble the changelog for 0.x.y".
user_invocable: true
allowed-tools: Bash Read Edit Write
---

# Assemble-changelog

Release-time changelog assembly for the Rerun repo.

Always work in the root of a standalone `rerun-io/rerun` checkout — normally the `prepare-release-0.x.y` branch, where the result is committed.
This is step 4 of [RELEASES.md](../../RELEASES.md); read it for the surrounding context.

Before doing any release work, verify that the current directory is the repository root and that its `origin` is `rerun-io/rerun`:

```bash
test "$(git rev-parse --show-toplevel)" = "$PWD"
git remote get-url origin
```

Do not run the workflow in the reality monorepo, including from its `rerun/` directory.
The release scripts need the standalone repository's `0.x.y` tags and resolve `(#N)` commit references against `rerun-io/rerun`.
Running them against reality can silently resolve reality PR numbers to unrelated Rerun PRs.

If either precondition is not met, stop before running any release command.
Tell the user that the skill requires the root of a standalone `rerun-io/rerun` checkout, and ask them to restart it there.
Do not clone a repository, fetch tags, or switch branches for the user.

Resolve the target version from `$ARGUMENTS` (e.g. `0.34.0`). If absent, read it
from `Cargo.toml` (`version = "0.x.y-…"`) and confirm with the user.

## Workflow

### 1. Assemble `upcoming/` → the release changeset

The curated entries live one-file-per-PR in `docs/content/changelog/upcoming/*.md`
(skip `_template.md`). Each declares `type: highlight|misc|breaking|feature` in its
frontmatter. Merge them into `docs/content/changelog/changeset-0-XX.md`, creating that file from
`docs/content/changelog/_template.md` if it does not exist yet (set `title` to the version — keep it
quoted, e.g. `title: "0.36"`, so YAML keeps it a string — and `order` one lower than the previous release):

- `highlight` → one `### ` subsection each under `## Highlights`.
- `feature`   → one `### ` subsection each under `## New features`.
- `misc`      → exactly one `- ` list item each under `## Other`. Fold its `### ` heading and body into the item, preserving links. Do not use headings or prose outside the list items.
- `breaking`  → one `### ` subsection each under `## Breaking changes`. If none, write `None.`.

Keep the sections in that order and omit `Highlights` and `Other` if they have no entries.
The changelog is user-facing (it's part of the website), so it leads with what's new;
the verbose, developer-only breaking-change migration guides go last so most readers
don't have to scroll past them.

Tailor the output to the release type:

- Patch release (`0.x.Y`, Y > 0) → typically only bug fixes. Skip `Highlights` and
  `New features` (there usually won't be `upcoming/` entries anyway); include `Other`
  only if there are miscellaneous entries, and keep `Breaking changes` only if there are any.
- Minor release (`0.X.0`) → the full template: highlights, new features, other items, breaking changes.

Preserve each entry's prose and structure (migration guides, tables, `snippet:` directives,
screenshots, links), except that each miscellaneous entry becomes its single list item.
De-duplicate overlapping entries and order breaking changes most-impactful first.
Drop the per-entry frontmatter.

Relative doc links in entries were written as if from `changelog/` (e.g.
`../reference/migration/...`), which is correct once merged — keep them as-is.

Finally, point the `redirect:` frontmatter in `docs/content/changelog.md` at
`changelog/changeset-0-XX`: CI's `scripts/ci/check_changelog_redirect.py` requires the
newest changeset to be the redirect target, so the repoint must land together with the
new changeset.

### 2. Resolve release blockers

Ensure that every non-template file from `upcoming/` was merged into the changeset, then search the assembled changeset for unresolved placeholders:

```bash
rg -n 'TODO\([^)]+\)' docs/content/changelog/changeset-0-XX.md # NOLINT
```

Resolve every match before continuing.
An unresolved `TODO(name)` blocks the release.

### 3. Generate the summary and detail sections into CHANGELOG.md

```bash
pixi run uvpy scripts/generate_changelog.py --version 0.x.y
```

Edit PR titles/labels to improve the output, then copy the result into `CHANGELOG.md`
(drop the trailing "Chronological changes" section; replace the placeholder video/blogpost
lines as previous releases did). Spot-check a few entries against the actual PRs:
polluted titles (old, unrelated PRs; `thanks @…` for core team members) mean a PR-number
lookup misfired — see the warning at the top.

Do this *after* step 1: the script reads the assembled changeset and emits a summary of it
(section headings + links to the changeset on the website), rather than inlining its prose.
`CHANGELOG.md` therefore never duplicates the changeset — if the changeset is missing, the
script emits an unresolved placeholder instead.

### 4. Polish the CHANGELOG.md section

The generated section is a starting point; edit it by hand so readers can find their way to the details.

- **Order the overview by impact.**
  Put the biggest new capabilities first, then integrations and APIs, then format support, then polish, then niche or developer-facing items.
- **Group and order the details.**
  The script emits each details subsection (🐍 Python API, 🪳 Bug fixes, 🌁 Viewer improvements, …) in commit order.
  Within each subsection, put entries that touch the same feature next to each other (e.g. all MCP/ViewerControl entries, all state timeline entries, all hangs and deadlocks), order the groups by their most impactful entry, and order entries within a group by impact.
  Only reorder lines; don't move entries between subsections or drop them.
- **Make entries clickable.**
  Wherever possible, link an entry (in the overview *and* in the details) to the docs that show how to use the feature: the how-to guide, the archetype/view reference page, the CLI or MCP reference section, the Python/JS ref site, or docs.rs.
  Reuse the links the changeset already contains, converted to `https://rerun.io/docs/<path without .md>`, and check that each target file and `#anchor` heading exists.
  Link external names too (e.g. a third-party tool to its docs).
  In detail entries, link the key term, not the whole line, so the trailing commit/PR link stays distinct.
- **Link each breaking change** to its subheading in the changeset: `https://rerun.io/docs/changelog/changeset-0-XX#<anchor>`.
  Anchors are the lowercased heading with backticks and punctuation dropped and spaces turned into hyphens (`rrd::optimize` → `rrdoptimize`).
- **Name the actual break.**
  A breaking-change heading (in the changeset and in `CHANGELOG.md`) must say what broke, e.g. "`WriteChunks` removed", not the new feature that replaces it ("Catalog staging").
  Lead the subsection with the removal and migration-guide link, then the replacement.
- **Flag experimental and unstable features.**
  Check every new feature in the changeset and `CHANGELOG.md` against these markers (`PREV` is the previous minor release tag, e.g. `0.38.0`):

  ```bash
  # Non-blueprint types marked unstable whose definitions changed this cycle
  git diff --name-only $PREV HEAD -- crates/build/re_type_definitions | grep -v /blueprint/ | xargs rg -l 'state = "unstable"'
  # Python APIs in the `rerun.experimental` module
  git log --oneline $PREV..HEAD -- rerun_py/rerun_sdk/rerun/experimental
  # Viewer features behind a flag in `ExperimentalAppOptions`
  git diff $PREV HEAD -- crates/viewer_support/re_viewer_context/src/app_options.rs
  # Docs and docstrings that call something experimental
  git diff --name-only $PREV HEAD -- docs/content rerun_py/rerun_sdk | xargs rg -l -i 'is experimental'
  ```

  A file showing up in the first command does not mean the type is new: cross-reference the hits with the actual entries.
  For each matching feature, say so where readers will see it: prefix the changeset heading with "Experimental" (as `### Experimental 3D gaussian splat support` did in 0.36), and add a sentence saying it may change in future releases (for unstable types: that the data may not stay backwards compatible).
  If the feature is behind a flag, say how to enable it.
  In the `CHANGELOG.md` overview, move all of these entries into a final `#### 🧪 Experimental and unstable` subsection at the end of "Overview & highlights", with a one-line note that they may change and that unstable types may not stay backwards compatible.
  Order that subsection by impact too, and don't repeat "Experimental" in its entries.
  Blueprint types (`rerun.blueprint.*`, under `re_type_definitions/rerun/blueprint/`) are an exception: they are all marked unstable, which only means the blueprint data isn't backwards compatible.
  That alone does not make a feature experimental, so a new view or blueprint setting is only experimental if one of the other markers applies to it (or to the data archetype it shows).
  Likewise, a Viewer feature is not experimental just because it is configured through an experimental API.
- **Add media to the headline features** in the overview: a screenshot (or a still frame plus a link to the video) indented under the bullet, using the `<picture>` markup that `pixi run upload-image` prints.
  Aim for a few images (roughly three to five), covering the most visual headline features: a picture sells a new view or renderer feature far better than a bullet does.
  Start from the media already in the changeset and the PR descriptions; if a visual feature has none, ask the user for a screenshot rather than skipping it.
  Don't add images to every entry: API, CLI, and bug-fix items rarely benefit.
  All media must live on `static.rerun.io`.
  Reuse a PR's media only if it is already on `static.rerun.io` or publicly accessible: `github.com/user-attachments/…` links from the private monorepo return 404 for the public.
  Download those with `curl -L -H "Authorization: token $(gh auth token)" <url>` and upload them with `pixi run upload-image <file> --name <name>`.
  GitHub does not render `<video>` from external hosts, so for a video, extract a representative still (`ffmpeg -ss <t> -i video.mp4 -frames:v 1 still.png`), upload it, and link the `.mp4` below it.
  Look at the frames and pick one without a cursor over the subject.

### 5. Empty the inbox

Delete the merged `upcoming/*.md` entries, keeping `_template.md` and any entries not included in this release.
Never add redirects for these temporary entries.

## Checklist before declaring done

- [ ] Every non-template `upcoming/` entry is represented in the changeset.
- [ ] No `TODO(name)` remains in the changeset.
- [ ] No summaries or other prose were synthesized for existing entries.
- [ ] The `CHANGELOG.md` overview is ordered by impact, a few of its most visual headline features have `static.rerun.io` images, and its breaking changes link to the changeset subheadings.
- [ ] Every overview and detail entry that has relevant docs links to them.
- [ ] Each details subsection groups entries by feature, most impactful first.
- [ ] Every experimental or unstable feature (feature flag, `rerun.experimental`, `#[rerun(state = "unstable")]` on a non-blueprint type, or documented as experimental) is labeled as such in the changeset, and listed under the overview's final "Experimental and unstable" subsection.
- [ ] `pixi run lint-rerun CHANGELOG.md docs/content/changelog/changeset-0-XX.md` passes.
- [ ] `upcoming/` contains only `_template.md` and entries deferred to a later release.
- [ ] `python scripts/ci/check_changelog_redirect.py` passes (redirect points at this changeset).
- [ ] `python scripts/ci/check_doc_redirects.py --base origin/main` passes (`upcoming/` entries are exempt and DO NOT need a redirect).

## Notes

- This skill lives in `skills/assemble-changelog` in the standalone Rerun repository.
- Synced commits in `rerun-io/rerun` carry a `Source-Ref` trailer (the reality merge
  commit); `generate_changelog.py` resolves it back to the originating reality PR for
  correct titles, labels, and contributors.
- The *next* release's changeset is not pre-created: an empty changeset for an
  unreleased version would make `check_changelog_redirect.py` fail, since it requires the newest
  `changeset-0-xx.md` to be the redirect target. During a cycle, `upcoming/` is the only in-flight
  artifact.
