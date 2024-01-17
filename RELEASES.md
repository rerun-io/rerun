# Releases and versioning
This document describes the current release and versioning strategy. This strategy is likely to change as Rerun matures.


## See also
* [`ARCHITECTURE.md`](ARCHITECTURE.md)
* [`BUILD.md`](BUILD.md)
* [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
* [`CODE_STYLE.md`](CODE_STYLE.md)
* [`CONTRIBUTING.md`](CONTRIBUTING.md)


## Release Cadence
New Rerun versions are released every four weeks. Sometimes we do out-of-schedule patch releases.


## Library versioning and release cadence
Each release include new versions of:
* All rust crates
* The Python SDK
* The Rust SDK
* The C++ SDK

We use semantic versioning. All versions are increased in lockstep, with a minor version bump each time (`0.1.0`, `0.2.0`, `0.3.0`, …).

This means we might add breaking changes in each new release.

In rare cases we will do patch releases, e.g. `0.3.1`, when there is a critical bug fix. These patch releases will not contain any breaking changes.

We sometimes do pre-releases. Then we use the versioning `0.2.0-alpha.0` etc.


## Data and communication versioning
We have not yet committed to any backwards or forwards compatibility.

We tag all data files (`.rrd` files) and communication protocols with the rerun version number. If there is a version mismatch, a warning is logged, but an attempt is still made to load the older or newer data.


## Releases
Release builds of the Python Wheels are triggered by pushing a release tag to GitHub in the form `0.2.0`.
If we are doing a patch release, we do a branch off of the latest release tag (e.g. `0.3.0`) and cherry-pick any fixes we want into that branch.

# Release process

1. ### Check the root [`Cargo.toml`](/Cargo.toml) to see what version we are currently on.

2. ### Create a release branch.

   The name should be:
   - `release-0.x.y` for final releases and their release candidates.
   - `release-0.x.y-alpha.N` where `N` is incremented from the previous alpha,
     or defaulted to `1` if no previous alpha exists.

![Image showing the branch create UI. You can find the `new branch` button at https://github.com/rerun-io/rerun/branches](https://github.com/rerun-io/rerun/assets/1665677/becaad03-9262-4476-b811-c23d40305aec)

Note: you do not need to create a PR for this branch -- the release workflow will do that for you.

3. ### If this is a patch release, cherry-pick commits for inclusion in the release into the branch.

4. ### Update [`CHANGELOG.md`](/CHANGELOG.md).

    It should include:
      - A one-line summary of the release
      - A multi-line summary of the release
      - A gif showing a major new feature
      - Run `pip install GitPython && scripts/generate_changelog.py`
      - Edit PR descriptions/labels to improve the generated changelog
      - Copy-paste the results into `CHANGELOG.md`.
      - Editorialize the changelog if necessary
      - Make sure the changelog includes instructions for handling any breaking changes

    Once you're done, commit and push the changelog onto the release branch.

5. ### Run the [release workflow](https://github.com/rerun-io/rerun/actions/workflows/release.yml).

   In the UI:
   - Set `Use workflow from` to the release branch you created in step (2).
   - Then choose one of the following values in the dropdown:
     - `alpha` if the branch name is `release-x.y.z-alpha.N`.
       This will create a one-off alpha release.

     - `rc` if the branch name is `release-x.y.z`.
       This will create a pull request for the release, and publish a release candidate.

     - `final` for the final public release

   ![Image showing the Run workflow UI. It can be found at https://github.com/rerun-io/rerun/actions/workflows/release.yml](https://github.com/rerun-io/rerun/assets/1665677/6cdc8e7e-c0fc-4cf1-99cb-0749957b8328)

6. ### Wait for the workflow to finish

   The PR description will contain next steps.
