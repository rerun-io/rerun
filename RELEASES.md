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
Go through this checklist from top to bottom, and check each item before moving onto the next.
This is a living document. Strive to improve it on each new release.

* [ ] Create a release branch called `release-0.x.y`
* [ ] If it is a patch release branch off `latest` and cherry-pick the commits that should be included
* [ ] Update `CHANGELOG.md` with the new version number with:
    * [ ] A one-line summary of the release
    * [ ] A multi-line summary of the release
    * [ ] A gif showing a major new feature
    * [ ] Run `pip install GitPython && scripts/generate_changelog.py`
    * [ ] Edit PR descriptions/labels to improve the generated changelog
    * [ ] Copy-paste the results into `CHANGELOG.md`.
    * [ ] Editorialize the changelog if necessary
    * [ ] Make sure the changelog includes instructions for handling any breaking changes
    * [ ] Commit and push the changelog
* [ ] Create a draft PR containing:
    * [ ] One-line summary of the release
    * [ ] A multi-line summary of the release
    * [ ] A gif showing a major new feature
* [ ] Test the branch ([see below](#testing-a-release))
* [ ] Open the PR up for review with the `⛴ release` label
* [ ] Bump version number in root `Cargo.toml`.
* [ ] Check that CI is green
* [ ] Publish the crates (see below)
* [ ] `git tag -a v0.x.y -m 'Release 0.x.y - summary'`
    * `git push --tags`
    * This will trigger a PyPI release when pushed
* [ ] `git pull --tags && git tag -d latest && git tag -a latest -m 'Latest release' && git push --tags origin latest --force`
* [ ] Manually trigger a new web viewer build and upload at https://github.com/rerun-io/rerun/actions/workflows/rust.yml
* [ ] Wait for CI to build release artifacts and publish them on GitHub and PyPI.
* [ ] Merge PR
* [ ] Edit the GitHub release to mark it at latest at the end of https://github.com/rerun-io/rerun/releases/edit/v0.x.0
* [ ] Wait for wheel to appear on https://pypi.org/project/rerun-sdk/
* [ ] Test the released Python and Rust libraries (see below)
* [ ] Wait for documentation to build: https://docs.rs/releases/queue
* [ ] Point <https://app.rerun.io/> to the latest release via instructions in <https://www.notion.so/rerunio/Ops-Notes-9232e436b80548a2b252c2312b4e4db6?pvs=4>.
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
  * [ ] Test the Python packages from PyPI: `pip install rerun_sdk==0.x.0a1`
  * [ ] Test rust install version: `cargo install -f rerun@0.x.0-alpha.1 -F web_viewer && rerun --web-viewer api.rrd`
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
