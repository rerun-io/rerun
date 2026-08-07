# Releases and versioning

This document describes the current release and versioning strategy. This strategy is likely to change as Rerun matures.

## See also

-   [`ARCHITECTURE.md`](ARCHITECTURE.md)
-   [`BUILD.md`](BUILD.md)
-   [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
-   [`CODE_STYLE.md`](CODE_STYLE.md)
-   [`CONTRIBUTING.md`](CONTRIBUTING.md)

## Repository

:warning: The steps & workflows in this document are targeting the open-source Rerun repository (https://github.com/rerun-io/rerun), not our internal monorepo.

## Release cadence

New Rerun versions are released every two weeks. Sometimes we do out-of-schedule patch releases.
We do not block a release on a PR. Incomplete work should be hidden behind a feature flag.

## Library versioning and release cadence

Each release include new versions of:

-   All Rust crates
-   The Python SDK
-   The Rust SDK
-   The C++ SDK

We use semantic versioning. All versions are increased in lockstep, with a minor version bump each time (`0.1.0`, `0.2.0`, `0.3.0`, …).

This means we might add breaking changes in each new release.

In rare cases we will do patch releases, e.g. `0.3.1`, when there is a critical bug fix. These patch releases will not contain any breaking changes.

We sometimes do pre-releases. Then we use the versioning `0.2.0-alpha.0` etc.

The version on our `main` branch is always an `-alpha.N+dev` version. We build artifacts from `main` every day, though these are not published on package registries.
You can find the latest development version in our [GitHub releases](https://github.com/rerun-io/rerun/releases/tag/prerelease).

## Rust version policy

Our Minimum Supported Rust Version (MSRV) is always _at least_ one minor release behind the latest Rust version, and ideally two releases.

-   This means users of our libraries aren't forced to update to the very latest Rust version
-   This lets us sometimes avoid new bugs in the newly released Rust compiler

## Data and communication versioning

We have not yet committed to any backwards or forwards compatibility.

We tag all data files (`.rrd` files) and communication protocols with the Rerun version number. If there is a version mismatch, a warning is logged, but an attempt is still made to load the older or newer data.
As of 0.23, we automatically migrate data from older versions, with an N-1 compatibility policy. That means `0.24` supports migrating `0.23` data, `0.25` supports migrating `0.24` data, etc.

# Release process

Before doing anything, read all the steps in full!

### 1. Determine what the next version should be

There usually isn't any ambiguity, as releases are planned ahead of time.

You can always find the latest release on our [GitHub releases](https://github.com/rerun-io/rerun/releases/tag/prerelease) page.

### 2. Create a release branch

The branch name is the single source of truth for the _release version_. Our release workflow automatically updates all versions
in the repository to what is specified in the branch name, so the format is important:

- `prepare-release-0.x.y` for minor and patch releases.
- `prepare-release-0.x.y-alpha.N` for alpha releases.

Note that `prepare-release-0.x` is _invalid_. Always specify the `y`, even if it is `0`, e.g. `prepare-release-0.15.0` instead of `prepare-release-0.15`.

The _base_ of the branch should depends on what kind of release it is:

- For a _minor_ release, the branch is created from `main`.
- For a _patch_ release, the branch is created from `docs-latest` ( :warning: branching off `docs-latest` instead of the latest release tag ensures that documentation patches will be included)
- For an _alpha_ release, the branch is created from `main`.

You can do this either using `git` on your command line, or through the UI:

![Image showing the branch create UI. You can find the `new branch` button at https://github.com/rerun-io/rerun/branches](https://github.com/rerun-io/rerun/assets/1665677/becaad03-9262-4476-b811-c23d40305aec)

Once the branch has been created, push it to the remote repository.

**NOTE (for patch releases)**: upon creation of the branch, the version is set to the final version of the previous release. This can create issues when testing the patch release, since it has a non-"+dev" version identical to/conflicting with an existing release. Because of that, an RC (release candidate) should preferably be created before testing.
The release workflow section below explains how you can create an RC.
Do this once you have prepared your patch-release branch and it's ready for testing.

### 3. If this is a patch release, cherry-pick commits for inclusion in the release into the branch

In GitHub we have a `consider-patch` label that we put on PRs that we might want to include in the release.
The fastest way to get an overview of all the patch candidate PRs from both repositories and their corresponding commit hashes is to run this script:

```
uv run scripts/fetch_patch_candidates.py
```

After cherry-picking a commit into the patch, please make sure to remove the `consider-patch` label.

### 4. Finalize the changeset and update [`CHANGELOG.md`](./CHANGELOG.md)

⚠️ Skip this step when preparing an alpha release.

The curated part of the release notes — highlights, breaking changes & migration guides, and new
features, including any screenshots/GIFs — is written incrementally **during the cycle, per PR**:
every `include in changelog` PR drops one file in [`docs/content/changelog/upcoming/`](./docs/content/changelog/upcoming)
(one file per PR avoids merge conflicts). At release, assemble those into the release's changeset:

-   Assemble `upcoming/*.md` into `docs/content/changelog/changeset-0-xx.md`, grouping each entry by
    its `type:` hint (`highlight` / `breaking` / `feature`). This is an agent-friendly task — run the
    `/assemble-changelog` skill (Claude Code), which merges and de-duplicates the entries, generates
    the detail sections, and verifies every `include in changelog` PR has an entry.
-   Once assembled and reviewed, delete the merged files from `upcoming/` (leave `_template.md`).
-   Resolve every `TODO(name)` in the changeset (docs links, example links, screenshots/GIFs, migration guides).
    **The release is blocked until all of these are resolved.**
-   Sanity-check the `## Highlights` section reads well as a whole. (Feature leads write their own items per PR;
    here you just make sure the one-line and multi-line summary of the release hang together.)

Then update `CHANGELOG.md` with the auto-generated detail (bug fixes + the full list of changes):

-   Run `pixi run uvpy scripts/generate_changelog.py --version 0.x.y > new_changelog.md`
-   Edit PR descriptions/labels to improve the generated changelog
-   Copy-paste the results into `CHANGELOG.md`.

The changeset is *not* copied into `CHANGELOG.md`. The script summarizes it instead — the section
headings plus links to the changeset on the website — so the full prose lives in exactly one place.
Assemble the changeset before running the script, or the summary will be a `TODO` placeholder. <!-- NOLINT -->

### 5. Clean up documentation links

⚠️ Skip this step when preparing an alpha release.

Remove all the `#[docs(unreleased)]` attributes in `re_type_definitions`, followed by `pixi run codegen`.

Remove the speculative link markers (`?speculative-link`).

Update the [python support table](./rerun_py/docs/gen_common_index.py) for the major release.

Point the `redirect:` in [`docs/content/changelog.md`](./docs/content/changelog.md) at this release's changeset.

Do *not* create the next release's changeset yet: `scripts/ci/check_changelog_redirect.py` requires the
newest `changeset-0-xx.md` to be the one `changelog.md` redirects to, so an empty changeset for an unreleased
version would fail CI. The next changeset is created from
[`docs/content/changelog/_template.md`](./docs/content/changelog/_template.md) when that release is assembled
(step 4). Until then the now-empty [`upcoming/`](./docs/content/changelog/upcoming) folder collects the next
release's per-PR entries.

Once you're done, commit and push onto the release branch.

### 6. Run the [release workflow](https://github.com/rerun-io/rerun/actions/workflows/release.yml)

In the UI:

-   Set `Use workflow from` to the release branch you created in step (2).
-   Then choose one of the following values in the dropdown:
    - `alpha` if the branch name is `prepare-release-x.y.z-alpha.N`.
        This will create a one-off alpha release.

    - `rc` if the branch name is `prepare-release-x.y.z`.
      This will publish a release candidate.

    - `final` for the final public release.

In all three cases, the workflow opens (or updates) a release pull request against `main`.

![Image showing the Run workflow UI. It can be found at https://github.com/rerun-io/rerun/actions/workflows/release.yml](https://github.com/rerun-io/rerun/assets/1665677/6cdc8e7e-c0fc-4cf1-99cb-0749957b8328)

### 7. Wait for the release workflow to finish

Once the release workflow is started, it will create a pull request for the release.
The pull request description will tell you what to do next.

[The `Release` workflow](https://github.com/rerun-io/rerun/actions/workflows/release.yml) will build artifacts, run PR checks, and publish them to PyPI, crates.io, npm, etc.
For `rc` and `final` releases it also creates a **draft** [GitHub release](https://github.com/rerun-io/rerun/releases) (in the `tag-release` job) and attaches a comment to the release PR pointing at it.

Once the `Release` workflow has finished successfully and you've sanity-checked the artifacts, edit the GitHub release draft (changelog, header media) and click `Publish release`.
Publishing the release triggers the [`GitHub Release` workflow](https://github.com/rerun-io/rerun/actions/workflows/on_gh_release.yml), which syncs the binary assets from `build.rerun.io` onto the published GitHub release.
**Make sure that workflow also finishes successfully** so the release ends up with all of its assets attached.

### 8. Merge changes to `main`

The release branch will contain a post-release version bump commit made by the release bot.
For example, `prepare-release-0.25.0` will be bumped to `0.26.0-alpha.1+dev` once everything has been released.
Additionally, it's common for us to push small changes and bug fixes directly to the release branch.

We want all of this to land back on `main`, so:

- For a minor release, merge the release branch to `main`.
- For a patch release, manually create a new PR from `main` and cherry-pick the commits. This includes at least
the `CHANGELOG.md` update, plus any other changes made on the release branch that haven't been cherry-picked in the
first place.
- For an alpha release, it's should be merged _if and only if_ the release job was successful.
  Otherwise, do not merge, as this could introduce breakage across the repository, such as in documentation links.
  If needed, cherry-pick any additional commits made back to `main`.

Make sure the `consider-patch` label on GitHub is up-to-date. For a full release, this usually means removing it from all PRs.

### 9. Optional: write a post mortem about the release

Summarize your experience with the release process to our [Release Postmortems](https://www.notion.so/rerunio/Release-Postmortems-271b24554b1980589770df810d2e4ed5) Notion page.

Create tickets if you think we can improve the process, put them into the `Actionable items` section.

### 10. Clean up PR labels

`uv run scripts/fetch_patch_candidates.py` will show a warning for `consider-patch`-labeled PRs that have been merged before a release.
Make sure to remove the label from PRs that are already part of a release.
