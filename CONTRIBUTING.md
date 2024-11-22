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

* **Bug reports and issues**: Open them at <https://github.com/rerun-io/rerun/issues>.

You can also look at our [`good first issue` tag](https://github.com/rerun-io/rerun/labels/good%20first%20issue).

## Pull requests
We use [Trunk Based Development](https://trunkbaseddevelopment.com/), which means we encourage small, short-lived branches.

Open draft PR:s to get some early feedback on your work until you feel it is ready for a proper review.
Do not make PR:s from your own `main` branch, as that makes it difficult for reviewers to add their own fixes.
Add any improvements to the branch as new commits to make it easier for reviewers to follow the progress (add images if possible!).

All PR:s are merged with [`Squash and Merge`](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/about-pull-request-merges#squash-and-merge-your-commits), meaning they all get squashed to just one commit on the `main` branch. This means you don't need to keep a clean commit history on your feature branches. In fact, it is preferable to add new commits to a branch rather than rebasing or squashing. For one, it makes it easier to track progress on a branch, but rebasing and force-pushing also discourages collaboration on a branch.

Our CI will [record various binary sizes](https://build.rerun.io/graphs/sizes.html) and run [some benchmarks](https://build.rerun.io/graphs/crates.html) on each merged PR.

Pull requests from external contributors require approval for CI runs. This can be done manually, by clicking the `Approve and run` button:

![Image showing the approve and run button](https://github.com/rerun-io/rerun/assets/1665677/ead5c04f-df02-4f20-9093-37cfce097b44)

Members of the `rerun-io` organization and collaborators in the `rerun-io/rerun` repository may enable auto-approval of workflow runs for a single PR by commenting with `@rerun-bot approve`:

![PR comment with the text `@rerun-bot approve`](https://github.com/rerun-io/rerun/assets/1665677/b5f07f3f-ea95-44a4-8eb7-f07c905f96c3)

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

## Tools

We use the [`pixi`](https://prefix.dev/) for managing dev-tool versioning, download and task running. To see available tasks, use `pixi task list`.

We use [cargo deny](https://github.com/EmbarkStudios/cargo-deny) to check our dependency tree for copy-left licenses, duplicate dependencies and [rustsec advisories](https://rustsec.org/advisories). You can configure it in `deny.toml`. Usage: `cargo deny check`
Configure your editor to run `cargo fmt` on save. Also configure it to strip trailing whitespace, and to end each file with a newline. Settings for VSCode can be found in the `.vscode` folder and should be applied automatically. If you are using another editor, consider adding good setting to this repository!

Depending on the changes you made run `cargo test --all-targets --all-features`, `pixi run py-test` and `pixi run -e cpp cpp-test` locally.

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
