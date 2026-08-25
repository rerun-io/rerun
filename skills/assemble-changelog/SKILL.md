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
(skip `_template.md`). Each declares `type: highlight|breaking|feature` in its
frontmatter. Merge them into `docs/content/changelog/changeset-0-XX.md`, creating that file from
`docs/content/changelog/_template.md` if it does not exist yet (set `title` to the version — keep it
quoted, e.g. `title: "0.36"`, so YAML keeps it a string — and `order` one lower than the previous release):

- `highlight` → fold into the `## Highlights` prose (write a cohesive few sentences
  selling the release; use the entries as raw material, don't just concatenate).
- `feature`   → one `### ` subsection each under `## New features`.
- `breaking`  → one `### ` subsection each under `## Breaking changes`. If none, write `None.`.

Keep the sections in that order. The changelog is user-facing (it's part of the website),
so it leads with what's new; the verbose, developer-only breaking-change migration guides
go last so most readers don't have to scroll past them.

Tailor the output to the release type:

- Patch release (`0.x.Y`, Y > 0) → typically only bug fixes. Skip `Highlights` and
  `New features` (there usually won't be `upcoming/` entries anyway); keep `Breaking
  changes` only if there are any.
- Minor release (`0.X.0`) → the full template: highlights, new features, breaking changes.

Preserve each entry's prose and structure (migration guides, tables, `snippet:` directives,
screenshots, links).
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

### 4. Empty the inbox

Delete the merged `upcoming/*.md` entries, keeping `_template.md`:

```bash
find docs/content/changelog/upcoming -maxdepth 1 -type f -name '*.md' ! -name '_template.md' -exec git rm -- {} +
```

Every entry was a published docs page, so each deletion needs a redirect in
`docs/content/_redirects.yaml` pointing at the section it was merged into, otherwise
`scripts/ci/check_doc_redirects.py` fails:

```yaml
# Changelog - 0.XX upcoming entries merged into the release changeset
changelog/upcoming/<slug>: changelog/changeset-0-XX#<heading-anchor>
```

The anchor is the `### ` heading lowercased with punctuation dropped and spaces turned into
dashes, so the heading `` ### `ParquetReader` loading options moved to `stream()` `` becomes
`parquetreader-loading-options-moved-to-stream`.

## Checklist before declaring done

- [ ] Every non-template `upcoming/` entry is represented in the changeset.
- [ ] No `TODO(name)` remains in the changeset.
- [ ] `## Highlights` reads as a coherent whole, not a list of fragments.
- [ ] `upcoming/` contains only `_template.md`.
- [ ] `python scripts/ci/check_changelog_redirect.py` passes (redirect points at this changeset).
- [ ] `python scripts/ci/check_doc_redirects.py --base origin/main` passes (every deleted `upcoming/` entry has a redirect).

## Notes

- This skill lives in `skills/assemble-changelog` in the standalone Rerun repository.
- Synced commits in `rerun-io/rerun` carry a `Source-Ref` trailer (the reality merge
  commit); `generate_changelog.py` resolves it back to the originating reality PR for
  correct titles, labels, and contributors.
- The *next* release's changeset is not pre-created: an empty changeset for an
  unreleased version would make `check_changelog_redirect.py` fail, since it requires the newest
  `changeset-0-xx.md` to be the redirect target. During a cycle, `upcoming/` is the only in-flight
  artifact.
