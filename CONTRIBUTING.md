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

## Pull Requests
We use [Trunk Based Development](https://trunkbaseddevelopment.com/), which means we encourage small, short-lived branches. Open draft PR:s to get some early feedback on your work.

All PR:s are merged with [`Squash and Merge`](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/about-pull-request-merges#squash-and-merge-your-commits), meaning they all get squashed to just one commit on the `main` branch. This means you don't need to keep a clean commit history on your feature branches. In fact, it is preferable to add new commits to a branch rather than rebasing or squashing. For one, it makes it easier to track progress on a branch, but rebasing and force-pushing also discourages collaboration on a branch.

Our CI will run benchmarks on each merged PR. The results can be found at <https://rerun-io.github.io/rerun/dev/bench/>.

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

We use the [`just`](https://github.com/casey/just) command runner tool for repository automation. See [here](https://github.com/casey/just#installation) for installation instructions. To see available automations, use `just --list`.

We use [cargo cranky](https://github.com/ericseppanen/cargo-cranky) and specify our clippy lints in [`Cranky.toml`](Cranky.toml). Usage: `cargo cranky`.

We use [cargo deny](https://github.com/EmbarkStudios/cargo-deny) to check our dependency tree for copy-left licenses, duplicate dependencies and [rustsec advisories](https://rustsec.org/advisories). You can configure it in `deny.toml`. Usage: `cargo deny check`.

Configure your editor to run `cargo fmt` on save. Also configure it to strip trailing whitespace, and to end each file with a newline. Settings for VSCode can be found in the `.vscode` folder and should be applied automatically. If you are using another editor, consider adding good setting to this repository!

To check everything in one go, run `./scripts/check.sh`. `check.sh` should ideally check approximately the same things as our CI.

### Optional
You can use [bacon](https://github.com/Canop/bacon) to automatically check your code on each save. For instance, running just `bacon` will re-run `cargo cranky` each time you change a rust file. See [`bacon.toml`](bacon.toml) for more.

You can set up [`sccache`](https://github.com/mozilla/sccache) to speed up re-compilation (e.g. when switching branches). You can control the size of the cache with `export SCCACHE_CACHE_SIZE="256G"`.

### Other
You can view higher log levels with `export RUST_LOG=debug` or `export RUST_LOG=trace`.
