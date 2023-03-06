# Releases and versioning
This document describes the current release and versioning strategy. This strategy is likely to change as Rerun matures.


## See also
* [`ARCHITECTURE.md`](ARCHITECTURE.md)
* [`BUILD.md`](BUILD.md)
* [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
* [`CODE_STYLE.md`](CODE_STYLE.md)
* [`CONTRIBUTING.md`](CONTRIBUTING.md)


## Release Cadence
New Rerun versions are released every two weeks. Sometimes we do out-of-schedule patch releases.


## Library versioning and release cadence
Each release include new versions of:
* The Python SDK
* The Rust SDK
* All rust crates

We use semantic versioning. All versions are increased in lockstep, with a minor version bump each time (`0.1.0`, `0.2.0`, `0.3.0`, …).

This means we might add breaking changes in each new release.

In rare cases we will do patch releases, e.g. `0.3.1`, when there is a critical bug fix. These patch releases will not contain any breaking changes.

We sometimes do pre-releases. Then we use the versioning `0.2.0-alpha.0` etc.


## Data and communication versioning
We have not yet committed to any backwards or forwards compatibility.

We tag all data files (`.rrd` files) and communication protocols with the rerun version number. If there is a version mismatch, a warning is logged, but an attempt is still made to load the older or newer data.


## Releases
Release builds of the Python Wheels are triggered by pushing a release tag to GitHub in the form `v0.2.0`.
If we are doing a patch release, we do a branch off of the latest release tag (e.g. `v0.3.0`) and cherry-pick any fixes we want into that branch.

### Release checklist
Copy this checklist to the the PR description, go through it from top to bottom, and check each item before moving onto the next. This is a living document. Strive to improve it on each new release.

* [ ] Create a release branch called `release-0.x.y`
* [ ] If it is a patch release branch off `latest` and cherry-pick the commits that should be included
* [ ] For the draft PR description, add a:
    * [ ] One-line summary of the release
    * [ ] A multi-line summary of the release
    * [ ] A gif showing a major new feature
* [ ] Test the branch ([see below](#testing-a-release))
* [ ] Open the PR up for review with the `⛴ release` label
* [ ] `./scripts/publish_crates.sh --dry-run`
* [ ] Bump version number in root `Cargo.toml`.
* [ ] Update `CHANGELOG.md` with the new version number and the summary and the gif
    * Go through https://github.com/rerun-io/rerun/compare/latest...HEAD and manually add any important PR descriptions to the `CHANGELOG.md`, with a link to the PR (which should have a deeper explanation).
      * You can use git log to quickly generate a list of commit headlines, use `git fetch --tags --force && git log --pretty=format:%s latest..HEAD` (fetch with `--force` is necessary to update the `latest` tag)
    * [ ] Make sure to it includes instructions for handling any breaking changes
* [ ] Get the PR reviewed
* [ ] Check that CI is green
* [ ] Publish the crates (see below)
* [ ] `git tag -a v0.x.y -m 'Release 0.x.y - summary'`
    * `git push --tags`
    * This will trigger a PyPI release when pushed
* [ ]  `git pull --tags && git tag -d latest && git tag -a latest -m 'Latest release' && git push --tags origin latest --force`
* [ ] Merge PR
* [ ] Wait for CI to build release artifacts and publish them on GitHub and PyPI. Verify this at https://github.com/rerun-io/rerun/releases/new.
* [ ] Wait for documentation to build: https://docs.rs/releases/queue
* [ ] Post on:
    * [ ] Community Discord
    * [ ] Rerun Twitter
    * [ ] Reddit?


### Testing a release
Before pushing the release tag:
  * [ ] `just py-run-all`
  * [ ] Test the web viewer:
      * [ ] `cargo run -p rerun --features web_viewer -- --web-viewer ../nyud.rrd`
      * [ ] Test on:
          * [ ] Chromium
          * [ ] Firefox
          * [ ] Mobile

After tagging and the CI has published:
  * [ ] Test the Python packages from PyPI: `pip install rerun_sdk==0.3.0a1`
  * [ ] Test rust install version: `cargo install -f rerun@0.3.0-alpha.1 -F web && rerun --web-viewer api.rrd`
  * [ ] Test rust crate: Modify Cargo.toml of any example to not point to the workspace
    * [ ] run with `--serve` to test web player

Checklist for testing alpha releases:
* Windows
  * [ ] Python Wheel
    * [ ] Web
    * [ ] Native
  * [ ] Rust crate
    * [ ] Web
    * [ ] Native
  * [ ] Rust install
    * [ ] Web
    * [ ] Native
* Linux
  * [ ] Python Wheel
    * [ ] Web
    * [ ] Native
  * [ ] Rust crate
    * [ ] Web
    * [ ] Native
  * [ ] Rust install
    * [ ] Web
    * [ ] Native
* Mac
  * [ ] Python Wheel
    * [ ] Web
    * [ ] Native
  * [ ] Rust crate
    * [ ] Web
    * [ ] Native
  * [ ] Rust install
    * [ ] Web
    * [ ] Native


## Publishing
First login as https://crates.io/users/rerunio with and API key you get from Emil:

```bash
cargo login $API_KEY
```

-----------------------------------------------------------------------------------------------
!! IMPORTANT !!  Shut off VSCode, and don't touch anything while `publish_crates.sh` is running!
!! IMPORTANT !!  Read `publish_crates.sh` for details
-----------------------------------------------------------------------------------------------

./scripts/publish_crates.sh --execute
