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

New Rerun versions are released approximately once every month. Sometimes we do out-of-schedule patch releases.

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

When done, run [`cargo semver-checks`](https://github.com/obi1kenobi/cargo-semver-checks) to check that we haven't introduced any semver breaking changes.

After cherry-picking a commit into the patch, please make sure to remove the `consider-patch` label.

### 4. Update [`CHANGELOG.md`](./CHANGELOG.md)

Update the change log. It should include:

-   A one-line summary of the release
-   A multi-line summary of the release
    - You may ask feature leads to write a summary for each highlighted item
-   A gif or screenshot showing one or more major new features
    - Try to avoid `mp4`s, gifs have a better experience on GitHub
    - You can upload images to a PR, use the link it generates to use GitHub as an image hosting service.
-   Run `pixi run uvpy scripts/generate_changelog.py > new_changelog.md`
-   Edit PR descriptions/labels to improve the generated changelog
-   Copy-paste the results into `CHANGELOG.md`.
-   Editorialize the changelog if necessary
-   Make sure the changelog includes instructions for handling any breaking changes

### 5. Clean up documentation links

Remove all the `attr.docs.unreleased` attributes in all `.fbs` files, followed by `pixi run codegen`.

Remove the speculative link markers (`?speculative-link`).

Update the [python support table](./rerun_py/docs/gen_common_index.py) for the major release.

Once you're done, commit and push onto the release branch.

### 6. Run the [release workflow](https://github.com/rerun-io/rerun/actions/workflows/release.yml)

In the UI:

-   Set `Use workflow from` to the release branch you created in step (2).
-   Then choose one of the following values in the dropdown:
    - `alpha` if the branch name is `prepare-release-x.y.z-alpha.N`.
        This will create a one-off alpha release.

    - `rc` if the branch name is `prepare-release-x.y.z`.
      This will create a pull request for the release, and publish a release candidate.

    - `final` for the final public release

![Image showing the Run workflow UI. It can be found at https://github.com/rerun-io/rerun/actions/workflows/release.yml](https://github.com/rerun-io/rerun/assets/1665677/6cdc8e7e-c0fc-4cf1-99cb-0749957b8328)

### 7. Wait for both workflows to finish

Once the release workflow is started, it will create a pull request for the release.
The pull request description will tell you what to do next.

[The `Release` workflow](https://github.com/rerun-io/rerun/actions/workflows/release.yml) will build artifacts and run PR checks.
Additionally, if the release type is set to `final` or `rc`, it will spawn a second workflow (when the release artifacts have been published to PyPI, crates.io etc.) called [`GitHub Release`](https://github.com/rerun-io/rerun/actions/workflows/on_gh_release.yml).
This workflow is responsible for creating [the GitHub release draft](https://github.com/rerun-io/rerun/releases) and to publish the artifacts to it.
**Make sure this workflow also finishes!**.
Only after it finishes successfully should you un-draft [the GitHub release](https://github.com/rerun-io/rerun/releases).

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
