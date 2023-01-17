# Releases and versioning
This document describes the current release and versioning strategy. This strategy is likely to change as Rerun matures.


## Cadence
New Rerun versions are released every two weeks. Sometimes we do out-of-schedule patch releases.


## Library versioning and release cadence
Each release include new versions of:
* The Python SDK
* The Rust SDK
* All rust crates

We use semantic versioning. All versions are increased in lockstep, with a minor version bump each time (`0.1.0`, `0.2.0`, `0.3.0`, …).

This means we might add breaking changes in each new release.

In rare cases we will do patch releases, e.g. `0.3.1`, when there is a critical bug fix. These patch releases will not contain any breaking changes.


## Data and communication versioning
We have not yet committed to any backwards or forwards compatibility.

We tag all data files (`.rrd` files) and communication protocols with the rerun version number. If there is a version mismatch, a warning is logged, but an attempt is still made to load the older or newer data.


## Releases
When doing a normal release, we create a branch off of `main`. If we are doing a patch release, we do a branch off of the latest release tag (e.g. `0.3.0`) and cherry-pick any fixes we want into that branch.

### Release checklist
Copy this checklist to the the PR description, go through it from top to bottom, and check each item before moving onto the next. This is a living document. Strive to improve it on each new release.

* [ ] Create a release branch called `release-0.x.y`
* [ ] If it is a patch release, cherry-pick the commits that should be included
* [ ] For the draft PR description, add a:
    * [ ] One-line summary of the release
    * [ ] A multi-line summary of the release
    * [ ] A gif showing a major new feature
* [ ] Test the branch ([see below](#testing-a-release))
* [ ] Open the PR up for review
* [ ] Bump version numbers
* [ ] Update `CHANGELOG.md` with the new version number and the summary and the gif
    * [ ] Make sure to it includes instructions for handling any breaking changes
* [ ] Get the PR reviewed
* [ ] Check that CI is green
* [ ] Publish new Rust crates
* [ ] Publish new Python wheels
* [ ] Merge PR
* [ ] `git tag -a 0.x.y -m 'Release 0.x.y - summary'`
* [ ] `git push && git push --tags`
* [ ] Do a GitHub release: https://github.com/rerun-io/rerun/releases/new
* [ ] Post on:
  * [ ] Community Discord
  * [ ] Rerun Twitter
  * [ ] Reddit?


### Testing a release
* [ ] `just py-run-all`
* [ ] Test the web viewer:
    * [ ] `cargo run -p rerun --features web -- --web-viewer ../nyud.rrd`
    * [ ] Test on:
        * [ ] Chromium
        * [ ] Firefox
        * [ ] Mobile


## To do before first release
* [ ] Add version numbers to `.rrd` files and communication protocols
* [ ] Find a tool that auto-updates our `CHANGELOG.md` on each merged PR
* [ ] See if we can use [`cargo-release`](https://github.com/crate-ci/cargo-release)
* [ ] Write instructions for how to publish the Python wheels
