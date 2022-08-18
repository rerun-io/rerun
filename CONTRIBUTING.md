# Contributing to Rerun
This is written for anyone who wants to contribute to the Rerun repository.

Rerun is an open core company, and this repository is dual-licensed under MIT and APACHE. However, this repository is NOT YET open source, but IT WILL BE. Therefore we ask you to avoid making public clones of this repository, but in other respects treat it as any other open source GitHub project.

## Setup
First up, you need to install the Rust toolchain: <https://rustup.rs/>. Then run `./setup_dev.sh`.

## Structure
The main crates are found in the `crates/` folder, with examples in the `examples/` folder.

Read about individual examples for details on how to run them.

To learn about the viewer, run:

```
cargo run --release -p rerun -- --help
```

## Tools
We use [cargo cranky](https://github.com/ericseppanen/cargo-cranky) and specify our clippy lints in `Cranky.toml`. Usage: `cargo cranky`.

We use [cargo deny](https://github.com/EmbarkStudios/cargo-deny) to check our dependency tree for copy-left licenses, duplicate dependencies and [rustsec advisories](https://rustsec.org/advisories). You can configure it in `deny.toml`. Usage: `cargo deny check`.

Configure your editor to run `cargo fmt` on save. Also configure it to strip trailing whitespace, an to end each file with a newline. Settings for VSCode can be found in the `.vscode` folder and should be applied automatically. If you are using another editor, consider adding good setting to this repository!

To check everything in one go, run `./check.sh`. `check.sh` should ideally check approximately the same things as our CI.

### Optional
You can use [bacon](https://github.com/Canop/bacon) to automatically check your code on each save. For instance, running just `bacon` will re-run `cargo cranky` each time you change a rust file. See `bacon.toml` for more.

### Other
You can view higher log levels with `export RUST_LOG=debug` or `export RUST_LOG=trace`.


## Style
We follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/about.html).

We use `rust fmt` with default settings.

We have blank lines before functions, types, `impl` blocks, and docstrings.

### Naming
When in doubt, be explicit. BAD: `id`. GOOD: `msg_id`.

Be terse when it doesn't hurt readability. BAD: `message_identifier`. GOOD: `msg_id`.

Avoid negations in names. A lot of people struggle with double negations, so things like `non_blocking = false` and `if !non_blocking { … }` can become a source of confusion and will slow down most readers. So prefer `connected` over `disconnected`, `initialized` over `uninitialized` etc.
