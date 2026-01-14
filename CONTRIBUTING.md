# Contributing to Rerun
This is written for anyone who wants to contribute to the Rerun repository.


## See also
* [`ARCHITECTURE.md`](ARCHITECTURE.md)
* [`BUILD.md`](BUILD.md)
* [`rerun_py/README.md`](rerun_py/README.md) - build instructions for Python SDK
* [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
* [`CODE_STYLE.md`](CODE_STYLE.md)
* [`RELEASES.md`](RELEASES.md)

## What to contribute
* **Examples**: We welcome any examples you would like to add. Follow the pattern of the existing examples in the [`examples/`](examples) folder.
* Report bugs and features requests at <https://github.com/rerun-io/rerun/issues>.
* You can also look at our [`good first issue` tag](https://github.com/rerun-io/rerun/labels/good%20first%20issue).
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
> PRs containing large changes which were not discussed previously may be closed without comment.

## Pull requests
We use [Trunk Based Development](https://trunkbaseddevelopment.com/), which means we encourage small, short-lived branches.

Open draft PR:s to get some early feedback on your work until you feel it is ready for a proper review.
Do not make PR:s from your own `main` branch, as that makes it difficult for reviewers to add their own fixes.
Add any improvements to the branch as new commits instead of rebasing to make it easier for reviewers to follow the progress (add images if possible!).

All PR:s are merged with [`Squash and Merge`](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/about-pull-request-merges#squash-and-merge-your-commits), meaning they all get squashed to just one commit on the `main` branch. This means you don't need to keep a clean commit history on your feature branches. In fact, it is preferable to add new commits to a branch rather than rebasing or squashing. For one, it makes it easier to track progress on a branch, but rebasing and force-pushing also discourages collaboration on a branch.

Our CI will [record various binary sizes](https://build.rerun.io/graphs/sizes.html) and run [some benchmarks](https://build.rerun.io/graphs/crates.html) on each merged PR.

Pull requests from external contributors require approval for CI runs. This can be done manually, by clicking the `Approve and run` button:

![Image showing the approve and run button](https://github.com/rerun-io/rerun/assets/1665677/ead5c04f-df02-4f20-9093-37cfce097b44)

Members of the `rerun-io` organization and collaborators in the `rerun-io/rerun` repository may enable auto-approval of workflow runs for a single PR by commenting with `@rerun-bot approve`:

![PR comment with the text `@rerun-bot approve`](https://github.com/rerun-io/rerun/assets/1665677/b5f07f3f-ea95-44a4-8eb7-f07c905f96c3)


### Labeling of PRs & changelog generation

Members of the `rerun-io` organization _have_ to add label PRs since they're part of how we generate [changelogs](https://github.com/rerun-io/rerun/blob/main/CHANGELOG.md).

If `include in changelog` is present, the **title** of the PR will be used as a line in the detailed section of the changelog.
Therefore, make sure that it's informative & concise without any additional context other than the category.
If a PR title should not be part of the changelog, you _have_ to label the PR with `exclude from changelog`.

The category of the changelog entry is governed by additional labels of which you have to provide at least one.
The exact list may change over time, see the [CI job](./.github/workflows/labels.yml) checking for it for a full list.

Beyond changelog categorization, it's encouraged to add too many rather than too few labels as they help with search.

#### What should go to the changelog?

Whether a PR should be added to the changelog isn't always clear cut, but it's advised to err on the side of
adding too many entries rather than too few!
Generally, whenever you believe that this is a value-add for a user browsing the detailed changelog, add it.
Also, credit where credit is due, be more generous adding contributions from outside the `rerun-io` org to the changelog!

We typically don't include:
pure refactors, testing, CI fixes, fixes for bugs that showed up since the last release, minor doc changes (like typos) etc.

#### Other special labels

* `deploy docs`:
  PRs marked with this will automatically be cherry-picked to the `docs-latest` branch which then will kick off a rebuild of the public [doc page](https://www.rerun.io/docs)
  Use this for landing doc fixes that are relevant to the latest release.
* `do-not-merge`:
  Will fail the CI unconditionally.
  Useful e.g. for opening PRs that target branches other than `main` which you want to rebase prior to merging
  or when you're still waiting on a test result.
  Alternatively, you can also use checkboxes in the PR description - any unticked checkboxes will make the ci fail automatically ✨

## Contributing to CI

Every CI job would in its ideal state consist of only a single `pixi` (or similar) script invocation that works locally as-is.

This approach has a number of benefits:
- Instead of Bash embedded in YAML, scripts may be written in an Actual Programming Language™
- Significantly lower iteration times when working on CI
- Ability to perform a job manually in case the CI fails

Additionally, always output any artifacts produced by CI to GCS instead of the GHA artifact storage. This can be a serious lifesaver when something breaks, as it allows anyone to download the output of a script and continue from where it failed, instead of being forced to start over from scratch.

Here are some guidelines to follow when writing such scripts:

Local-first means easy for contributors to run.

The following should be documented in each script:
- Dependencies
- Files and directories
- Environment variables
- Usage examples

Inputs should be passed in explicitly via arguments, and use sane defaults. If an input has a default value, it should be documented in its description.

Every input should be checked as early as possible. This includes:
- Checking if authentication credentials are valid
- Validating inputs and parsing into more specific types where possible:
  - Numeric ranges
  - String character sets/encodings
  - Length limits
  - Date formats
  - etc.
- Checking that input file paths are valid and the files they point to exist

Input and output file paths should also accept GCS paths (`gs://bucket/blob/path`) and stdin/stdout (`-`), if it makes sense.

Be extra descriptive in error messages, it may be the only piece of information someone debugging a CI failure has available to figure out what went wrong. Print frequently to hint at what is going on and display progress to the user.

Environment variables should only be used for authentication with external services and configuring output (e.g. disabling color). Many SDKs support some form of persistent/default authentication, and scripts should take advantage of this where possible. For example, GCP has [Application Default Credentials](https://cloud.google.com/docs/authentication/client-libraries).

If the script performs destructive or otherwise irreversible actions, then it should support a `--dry-run` option if possible.

### Adding dependencies
Be thoughtful when adding dependencies. Each new dependency is a liability which lead to increased compile times, a bigger binary, more code that can break, a larger attack surface, etc. Sometimes it is better to write a hundred lines of code than to add a new dependency.

Whenever you add a new dependency in a PR, make sure you motivate it:
* Why use the dependency instead of rolling our own?
* Why this dependency instead of another?

For Rust, make sure you use `default-features = false` if it makes sense, to minimize the amount of new code that is pulled in.

When reviewing a PR, always check the diff of `Cargo.lock` (it is collapsed by default in GitHub 😤).

For a guide on picking good dependencies, see <https://gist.github.com/repi/d98bf9c202ec567fd67ef9e31152f43f>.

Any full `cargo update` should be its own stand-alone PR. Make sure you include the output of it in the commit message.


## Structure
The main crates are found in the [`crates/`](crates) folder, with examples in the [`examples/`](examples) folder.

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
If not noted otherwise, all tests run automated on CI, however their frequency (per PR, on `main`, nightly, etc.) and platform coverage may vary.

### Rust tests

```sh
cargo test --all-targets --all-features
```
or alternatively (if you've [installed cargo nextest](https://nexte.st/)):
```sh
cargo nextest run --all-targets --all-features
cargo test --all-features --doc
```

Runs both unit & integration tests for Rust crates, including the Rerun viewer.

Tests are written using the standard `#[test]` attribute.

#### `insta` snapshot tests

Some of the tests in the `rerun` family of crates are [`insta`](https://docs.rs/insta/latest/insta/) snapshot tests.
These tests work by comparing a textual output of a test against a checked-in reference.

They run as part of the regular Rust test suite, no extra action is required to include them in a test run.

If the output of them changes (either intentionally or not), they will fail, and you can review the results by running `cargo insta review` (you first need to install it with `cargo install cargo-insta`).

#### Image comparison tests

Some of the tests in the `rerun` family of crates are image comparison tests.
These tests work by rendering an image and then comparing it with a checked-in reference image.

They run as part of the regular Rust test suite, no extra action is required to include them in a test run.

Comparison tests are driven by [egui_kittest](https://github.com/emilk/egui/tree/master/crates/egui_kittest)'s `Harness::snapshot` method.
Typically, we use [TestContext](./crates/viewer/re_test_context/src/lib.rs) in order to mock
relevant parts of the Rerun viewer.

##### Comparing results & updating images

Each run of the comparison tests will produce new images that are saved to the comparison images.
(typically at `<your-test.rs>/snapshots`)

Upon failure, additionally `diff.png` file is added that highlights all differences between the reference and the new image.
In order to update reference with the new image, run with `UPDATE_SNAPSHOTS=1` environment variable set.

Use `pixi run snapshots` to compare the results of all failed tests in Rerun.

For best practices & unexpected sources of image differences refer to the [egui_kittest README](https://github.com/emilk/egui/tree/master/crates/egui_kittest#snapshot-testing).

##### Rendering backend

Just like for drawing the viewer itself, drawing for comparison tests requires a `wgpu` compatible driver.
As of writing comparison tests are only run via Vulkan & Metal.
For CI / headless environments we a recent version `llvmpipe` for software rendering on Linux & Windows.
On MacOS we use [`SwiftShader`](https://github.com/google/swiftshader/).

⚠️ Unfortunately, `SwiftShader`'s MSAA & texture filtering differs drastically from `llvmpipe` and
other native renderers which is why we use a lot higher comparison treshholds on Mac.
-> **DO NOT** use images generated on MacOS CI as reference image, prefer those produced by our Linux runner.

For details on how to set this up refer to the [CI setup](./.github/workflows/reusable_checks_rust.yml).


### Python tests

```sh
pixi run py-test
```

The Python SDK is tested using [`pytest`](https://docs.pytest.org/).
Tests are located in the [./rerun_py/tests/](./rerun_py/tests/) folder.

### C++ tests

```sh
pixi run cpp-test
```

The C++ SDK is tested using [`catch2`](https://github.com/catchorg/Catch2).
Tests are located in the [./rerun_cpp/tests/](./rerun_cpp/tests/) folder.


### Snippet comparison tests

```sh
pixi run uvpy docs/snippets/compare_snippet_output.py
```

More details in the [README.md](./docs/snippets/README.md).

Makes sure all of the snippets in the [snippets/](./docs/snippets/) folder are working and yield the same output in all of the supported languages, unless configured otherwise in the [snippets.toml](./docs/snippets/snippets.toml) file.

### Release checklists

```sh
pixi run uv run tests/python/release_checklist/main.py
```

More details in the [README.md](./tests/python/release_checklist/README.md).

A set of **manual** checklist-style tests that should be run prior to each release.
Introduction of new release checklists should be avoided as they add a lot of friction to the release process,
and failures are easy to be missed.

### Other ad-hoc manual tests

There's various additional test scenes located at [./tests/cpp/](./tests/cpp/), [./tests/python/](./tests/python/) and [./tests/rust/](./tests/rust/).
We generally build those as a CI step, but they are run only irregularly.
See respective readme files for more details.

## Tools

We use the [`pixi`](https://pixi.sh/) for managing dev-tool versioning, download and task running. To see available tasks, use `pixi task list`.

We use [cargo deny](https://github.com/EmbarkStudios/cargo-deny) to check our dependency tree for copy-left licenses, duplicate dependencies and [rustsec advisories](https://rustsec.org/advisories). You can configure it in `deny.toml`. Usage: `cargo deny check`
Configure your editor to run `cargo fmt` on save. Also configure it to strip trailing whitespace, and to end each file with a newline. Settings for VSCode can be found in the `.vscode` folder and should be applied automatically. If you are using another editor, consider adding good setting to this repository!

Depending on the changes you made run `cargo test --all-targets --all-features`, `pixi run py-test` and `pixi run -e cpp cpp-test` locally.
For details see [the test section above](#tests).

It is not strictly required, but we recommend [`cargo nextest`](https://nexte.st/) for running Rust tests as it is significantly faster than `cargo test` and yields much more readable output.
Note however, that as of writing `cargo nextest` does not yet support doc tests, those need to be run with `cargo test`.

### Linting
Prior to pushing changes to a PR, at a minimum, you should always run `pixi run fast-lint`. This is designed to run
in a few seconds for repeated runs and should catch the more trivial issues to avoid wasting CI time.

### Hooks
We recommend adding the Rerun pre-push hook to your local checkout, which among other-things will run
`pixi run fast-lint` for you.

To install the hooks, simply copy them into the `.git/hooks` directory of your local checkout.
```
cp hooks/pre-push .git/hooks/pre-push
chmod +x .git/hooks/pre-push
```
or if you prefer you can configure git to use this directory as the hooks directory:
```
git config core.hooksPath hooks
```

### Optional
You can use [bacon](https://github.com/Canop/bacon) to automatically check your code on each save. For instance, running just `bacon` will re-run `cargo clippy` each time you change a Rust file. See [`bacon.toml`](bacon.toml) for more.

You can set up [`sccache`](https://github.com/mozilla/sccache) to speed up re-compilation (e.g. when switching branches). You can control the size of the cache with `export SCCACHE_CACHE_SIZE="256G"`.

### Other
You can view higher log levels with `export RUST_LOG=trace`.
Debug logging is automatically enabled for the viewer as long as you're running inside the `rerun` checkout.
