# Contributing to Rerun
This guide is for anyone who wants to contribute to the Rerun repository.


## See also
* [`ARCHITECTURE.md`](ARCHITECTURE.md)
* [`BUILD.md`](BUILD.md)
* [`rerun_py/README.md`](rerun_py/README.md) - build instructions for Python SDK
* [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
* [`CODE_STYLE.md`](CODE_STYLE.md)
* [`RELEASES.md`](RELEASES.md)

## What to contribute
* **Examples**: We welcome any examples you would like to add. Follow the pattern of existing examples in the [`examples/`](examples) folder.
* Report bugs and feature requests at <https://github.com/rerun-io/rerun/issues>.
* Look at our [`good first issue` tag](https://github.com/rerun-io/rerun/labels/good%20first%20issue).
* We track things we would like implemented in 3rd party crates [here](https://github.com/rerun-io/opensource/issues/1).

Note that maintainers do not have infinite time, and reviews take a lot of it.
When choosing what to work on, please ensure that it is either:

* A small change (+100-100 at most), or
* A larger change that has been discussed with one or more maintainers.

You can discuss these changes by:

* Commenting on an existing issue,
* Creating a new issue, or
* Pinging one of the Rerun maintainers on our [Discord](https://discord.gg/PXtCgFBSmH)

> [!NOTE]
> PRs containing large undiscussed changes may be closed without comment.

## Pull requests
We use [Trunk Based Development](https://trunkbaseddevelopment.com/), which means we encourage small, short-lived branches.

* Open draft PRs early to get feedback before a full review.
* Don't PR from your own `main` branch — it makes it hard for reviewers to add fixes.
* Add improvements as new commits rather than rebasing, so reviewers can follow progress (add images if possible!).
* All PRs are merged with [`Squash and Merge`](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/about-pull-request-merges#squash-and-merge-your-commits), so you don't need a clean commit history on feature branches. Prefer new commits over rebasing — force-pushing discourages collaboration.

Our CI will [record binary sizes](https://build.rerun.io/graphs/sizes.html) and run [benchmarks](https://build.rerun.io/graphs/crates.html) on each merged PR.

Pull requests from external contributors require approval for CI runs. Click the `Approve and run` button:

![Image showing the approve and run button](https://github.com/rerun-io/rerun/assets/1665677/ead5c04f-df02-4f20-9093-37cfce097b44)

Members of the `rerun-io` organization can enable auto-approval for a single PR by commenting with `@rerun-bot approve`:

![PR comment with the text `@rerun-bot approve`](https://github.com/rerun-io/rerun/assets/1665677/b5f07f3f-ea95-44a4-8eb7-f07c905f96c3)


### Labeling of PRs & changelog generation

Org members _must_ label their PRs — labels are how we generate [changelogs](https://github.com/rerun-io/rerun/blob/main/CHANGELOG.md).

* `include in changelog`: The PR **title** will be used as a changelog entry. Keep it informative and concise.
* `exclude from changelog`: Required if the PR shouldn't appear in the changelog.
* At least one category label is required. See the [CI job](./.github/workflows/labels.yml) for the current list.
* When in doubt, add more labels rather than fewer — they help with search.

#### What should go to the changelog?

Err on the side of including entries — if it adds value for a user browsing the changelog, add it.
Be generous with external contributions — credit where credit is due!

We typically don't include: pure refactors, testing, CI fixes, fixes for bugs introduced since last release, minor doc changes (typos, etc.).

#### Other special labels

* `deploy docs`:
  Cherry-picked to `docs-latest`, triggering a rebuild of the [doc page](https://www.rerun.io/docs).
  Use this for doc fixes relevant to the latest release.
* `do-not-merge`:
  Fails CI unconditionally. Useful for PRs targeting non-`main` branches or awaiting test results.
  Alternatively, unticked checkboxes in the PR description will also fail CI ✨

## Contributing to CI

Every CI job should ideally be a single `pixi` (or similar) script invocation that works locally as-is.

Benefits:
- Scripts in a real programming language instead of Bash embedded in YAML
- Much lower iteration times when working on CI
- Ability to manually re-run a job when CI fails

Always output artifacts to GCS instead of GHA artifact storage. This lets anyone download the output of a script and continue from where it failed.

### CI script guidelines

Scripts should be local-first and easy for contributors to run.

Each script should document:
- Dependencies
- Files and directories
- Environment variables
- Usage examples

Pass inputs explicitly via arguments with sane defaults. Validate inputs as early as possible: auth credentials, numeric ranges, string formats, file path existence, etc.

Support GCS paths (`gs://bucket/blob/path`) and stdin/stdout (`-`) for file I/O where it makes sense.

Write descriptive error messages — they may be the only info someone has when debugging a CI failure. Print frequently to show progress.

Use environment variables only for auth and output config (e.g. disabling color). Prefer SDK default auth where possible (e.g. GCP [Application Default Credentials](https://cloud.google.com/docs/authentication/client-libraries)).

Support `--dry-run` for destructive or irreversible actions.

### Adding dependencies
Be thoughtful when adding dependencies. Each one adds compile time, binary size, potential breakage, and attack surface. Sometimes 100 lines of code is better than a new dependency.

When adding a dependency in a PR, motivate it:
* Why use this dependency instead of rolling our own?
* Why this one over alternatives?

For Rust, use `default-features = false` where it makes sense to minimize new code pulled in.

When reviewing a PR, always check the `Cargo.lock` diff (collapsed by default in GitHub 😤).

Guide for picking good dependencies: <https://gist.github.com/repi/d98bf9c202ec567fd67ef9e31152f43f>.

A full `cargo update` should be its own stand-alone PR. Include the output in the commit message.


## Structure
Main crates are in [`crates/`](crates), examples in [`examples/`](examples).

To get an overview of the crates, read their documentation with:

```
cargo doc --no-deps --open
```

To learn about the viewer, run:

```
cargo run -p rerun -- --help
```

## Tests

There are various kinds of automated tests throughout the repository.
Unless noted otherwise, all tests run on CI, though their frequency (per PR, on `main`, nightly) and platform coverage may vary.

### Rust tests

```sh
cargo test --all-targets --all-features
```
or with [cargo nextest](https://nexte.st/):
```sh
cargo nextest run --all-targets --all-features
cargo test --all-features --doc
```

Runs unit & integration tests for all Rust crates, including the viewer.
Tests use the standard `#[test]` attribute.

#### `insta` snapshot tests

Some tests use [`insta`](https://docs.rs/insta/latest/insta/) snapshot tests, which compare textual output against checked-in references. They run as part of the regular test suite.

If output changes, they will fail. Review results with `cargo insta review` (install: `cargo install cargo-insta`).

#### Image comparison tests

Some tests render an image and compare it against a checked-in reference image. They run as part of the regular test suite.

These are driven by [egui_kittest](https://github.com/emilk/egui/tree/master/crates/egui_kittest)'s `Harness::snapshot` method.
We typically use [TestContext](./crates/viewer/re_test_context/src/lib.rs) to mock relevant parts of the viewer.

##### Comparing results & updating images

Each test run produces new images (typically at `<your-test.rs>/snapshots`).
On failure, a `diff.png` is added highlighting all differences.
To update references, run with `UPDATE_SNAPSHOTS=1`.

Use `pixi run snapshots` to compare results of all failed tests visually in Rerun.
You can also update from a failed CI run using `./scripts/update_snapshots_from_ci.sh`.
Inspect PR diffs (including failed comparisons) via https://rerun-io.github.io/kitdiff/?url=<link to GitHub PR>.

For best practices and unexpected sources of image differences, see the [egui_kittest README](https://github.com/emilk/egui/tree/master/crates/egui_kittest#snapshot-testing).

##### Rendering backend

Image comparison tests require a `wgpu`-compatible driver. Currently they run on Vulkan & Metal.
For CI / headless environments, we use lavapipe (`llvmpipe`) for software rendering on all platforms.
On macOS, we use a custom static build from [`rerun-io/lavapipe-build`](https://github.com/rerun-io/lavapipe-build).

For setup details, see the [CI workflow](./.github/workflows/reusable_checks_rust.yml).


### Python tests

```sh
pixi run py-test
```

Uses [`pytest`](https://docs.pytest.org/). Tests are in [./rerun_py/tests/](./rerun_py/tests/).

### C++ tests

```sh
pixi run cpp-test
```

Uses [`catch2`](https://github.com/catchorg/Catch2). Tests are in [./rerun_cpp/tests/](./rerun_cpp/tests/).


### Snippet comparison tests

```sh
pixi run uvpy docs/snippets/compare_snippet_output.py
```

Verifies that all [snippets](./docs/snippets/) produce the same output across languages, unless configured otherwise in [snippets.toml](./docs/snippets/snippets.toml). More details in [README.md](./docs/snippets/README.md).

### Release checklists

```sh
pixi run uv run tests/python/release_checklist/main.py
```

A set of **manual** checklist-style tests run prior to each release. Avoid adding new ones — they add friction and failures are easy to miss. More details in [README.md](./tests/python/release_checklist/README.md).

### Other ad-hoc manual tests

Additional test scenes in [./tests/cpp/](./tests/cpp/), [./tests/python/](./tests/python/), and [./tests/rust/](./tests/rust/).
These are built on CI but run only irregularly. See respective READMEs for details.

## Tools

We use [`pixi`](https://pixi.sh/) for dev-tool versioning, downloads, and task running. See available tasks with `pixi task list`.

We use [cargo deny](https://github.com/EmbarkStudios/cargo-deny) to check our dependency tree for copyleft licenses, duplicate dependencies, and [rustsec advisories](https://rustsec.org/advisories). Configure in `deny.toml`, run with `cargo deny check`.

Configure your editor to run `cargo fmt` on save, strip trailing whitespace, and end each file with a newline. VSCode settings in `.vscode/` should apply automatically. If you use a different editor, consider adding good settings to this repository!

Run relevant tests locally depending on your changes: `cargo test --all-targets --all-features`, `pixi run py-test`, `pixi run -e cpp cpp-test`. See [Tests](#tests) for details.

We recommend [`cargo nextest`](https://nexte.st/) for running Rust tests — it's faster than `cargo test` with better output. Note that it doesn't support doc tests yet; run those with `cargo test`.

### Linting
Before pushing, always run `pixi run fast-lint`. It takes seconds on repeated runs and catches trivial issues before wasting CI time.

### Hooks
We recommend installing the Rerun pre-push hook, which runs `pixi run fast-lint` for you.

Copy it into your local `.git/hooks`:
```
cp hooks/pre-push .git/hooks/pre-push
chmod +x .git/hooks/pre-push
```
or configure git to use the hooks directory directly:
```
git config core.hooksPath hooks
```

### Optional
* [bacon](https://github.com/Canop/bacon) — automatically re-runs `cargo clippy` on save. See [`bacon.toml`](bacon.toml).
* [`sccache`](https://github.com/mozilla/sccache) — speeds up recompilation (e.g. when switching branches). Set cache size: `export SCCACHE_CACHE_SIZE="256G"`.

### Other
View higher log levels with `export RUST_LOG=trace`.
Debug logging is automatically enabled for the viewer when running inside the `rerun` checkout.
