# Releases and versioning
This document describes the current release and versioning strategy. This strategy is likely to change as Rerun matures.


## See also
* [`ARCHITECTURE.md`](ARCHITECTURE.md)
* [`BUILD.md`](BUILD.md)
* [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
* [`CODE_STYLE.md`](CODE_STYLE.md)
* [`CONTRIBUTING.md`](CONTRIBUTING.md)


## Release cadence
New Rerun versions are released approximately once every month. Sometimes we do out-of-schedule patch releases.


## Library versioning and release cadence
Each release include new versions of:
* All Rust crates
* The Python SDK
* The Rust SDK
* The C++ SDK

We use semantic versioning. All versions are increased in lockstep, with a minor version bump each time (`0.1.0`, `0.2.0`, `0.3.0`, …).

This means we might add breaking changes in each new release.

In rare cases we will do patch releases, e.g. `0.3.1`, when there is a critical bug fix. These patch releases will not contain any breaking changes.

We sometimes do pre-releases. Then we use the versioning `0.2.0-alpha.0` etc.


## Rust version policy
Our Minimum Supported Rust Version (MSRV) is always _at least_ one minor release behind the latest Rust version, and ideally two releases.
* This means users of our libraries aren't forced to update to the very latest Rust version
* This lets us sometimes avoid new bugs in the newly released Rust compiler


## Data and communication versioning
We have not yet committed to any backwards or forwards compatibility.

We tag all data files (`.rrd` files) and communication protocols with the Rerun version number. If there is a version mismatch, a warning is logged, but an attempt is still made to load the older or newer data.


## Releases
Release builds of the Python Wheels are triggered by pushing a release tag to GitHub in the form `0.2.0`.
If we are doing a patch release, we do a branch off of the latest release tag (e.g. `0.3.0`) and cherry-pick any fixes we want into that branch.

# Release process

### 1. Check the root [`Cargo.toml`](/Cargo.toml) to see what version we are currently on.

### 2. Create a release branch.

The name should be:
- `release-0.x.y` for final releases and their release candidates.
- `release-0.x.y-alpha.N` where `N` is incremented from the previous alpha,
  or defaulted to `1` if no previous alpha exists.

Note that `release-0.x` is _invalid_. Always specify the `y`, even if it is `0`,
e.g. `release-0.15.0` instead of `release-0.15`.

For minor release, the branch is typically created from `main`. For patch release, the branch is typically created
from the previous release's tag.

![Image showing the branch create UI. You can find the `new branch` button at https://github.com/rerun-io/rerun/branches](https://github.com/rerun-io/rerun/assets/1665677/becaad03-9262-4476-b811-c23d40305aec)

Note: you do not need to create a PR for this branch -- the release workflow will do that for you.

### 3. If this is a patch release, cherry-pick commits for inclusion in the release into the branch.

When done, run [`cargo semver-checks`](https://github.com/obi1kenobi/cargo-semver-checks) to check that we haven't introduced any semver breaking changes.

:warning: Any commits between the last release's tag and the `docs-latest` branch should also be cherry-picked,
otherwise these changes will be lost when `docs-latest` is updated.

```
# On branch `release-0.x.y`
git fetch origin docs-latest:docs-latest
git cherry-pick 0.x.z..docs-latest
```

Where `z` is the previous patch number.

Note that the `cherry-pick` will fail if there are no additional `docs-latest` commits to include,
which is fine.

### 4. Update [`CHANGELOG.md`](/CHANGELOG.md) and clean ups.

Update the change log. It should include:
  - A one-line summary of the release
  - A multi-line summary of the release
  - A gif showing a major new feature
  - Run `pip install GitPython && scripts/generate_changelog.py > new_changelog.md`
  - Edit PR descriptions/labels to improve the generated changelog
  - Copy-paste the results into `CHANGELOG.md`.
  - Editorialize the changelog if necessary
  - Make sure the changelog includes instructions for handling any breaking changes

Remove the speculative link markers (`?speculative-link`).

Find all `"attr.docs.state": "unreleased"` in `.fbs` files and change it to either "experimental", "unstable", or "stable". Run codegen.

Once you're done, commit and push onto the release branch.

### 5. Run the [release workflow](https://github.com/rerun-io/rerun/actions/workflows/release.yml).

In the UI:
- Set `Use workflow from` to the release branch you created in step (2).
- Then choose one of the following values in the dropdown:
  - `alpha` if the branch name is `release-x.y.z-alpha.N`.
    This will create a one-off alpha release.

  - `rc` if the branch name is `release-x.y.z`.
    This will create a pull request for the release, and publish a release candidate.

  - `final` for the final public release

![Image showing the Run workflow UI. It can be found at https://github.com/rerun-io/rerun/actions/workflows/release.yml](https://github.com/rerun-io/rerun/assets/1665677/6cdc8e7e-c0fc-4cf1-99cb-0749957b8328)

### 6. Wait for the workflow to finish

The PR description will contain next steps.

Note: there are two separate workflows running -- the one building the release artifacts, and the one running the PR checks.
You will have to wait for the [former](https://github.com/rerun-io/rerun/actions/workflows/release.yml) in order to get a link to the artifacts.

### 7. Merge changes to `main`

For minor release, merge the release branch to `main`.

For patch release, manually create a new PR from `main` and cherry-pick the required commits. This includes at least
the `CHANLGE.log` update, plus any other changes made on the release branch that hasn't been cherry-picked in the
first place.
