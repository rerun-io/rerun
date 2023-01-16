# Contributing to Rerun
This is written for anyone who wants to contribute to the Rerun repository.

Rerun is an open core company, and this repository is dual-licensed under MIT and APACHE. However, this repository is NOT YET open source, but IT WILL BE. Therefore we ask you to avoid making public clones of this repository, but in other respects treat it as any other open source GitHub project.

## What to contribute
* **Examples**: We welcome any examples you would like to add. Follow the pattern of the existing examples in the [`examples/`](examples) folder.

* **Bug reports and issues**: Open them at <https://github.com/rerun-io/rerun/issues>.

You can also look at our [`good first issue` tag](https://github.com/rerun-io/rerun/labels/good%20first%20issue).

## Pull Requests
We use [Trunk Based Development](https://trunkbaseddevelopment.com/), which means we encourage small, short-lived branches. Open draft PR:s to get some early feedback on your work.

All PR:s are merged with [`Squash and Merge`](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/incorporating-changes-from-a-pull-request/about-pull-request-merges#squash-and-merge-your-commits), meaning they all get squashed to just one commit on the `main` branch. This means you don't need to keep a clean commit history on your feature branches. In fact, it is preferable to add new commits to a branch rather than rebasing or squashing. For one, it makes it easier to track progress on a branch, but rebasing and force-pushing also discourages collaboration on a branch.

## Getting started with the repository.
* Install the Rust toolchain: <https://rustup.rs/>
* `git clone git@github.com:rerun-io/rerun.git && cd rerun`
* Run `./scripts/setup_dev.sh`.
* Make sure `cargo --version` prints `1.65.0` once you are done

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

### Other
You can view higher log levels with `export RUST_LOG=debug` or `export RUST_LOG=trace`.

## Rust code

### Error handling and logging
We log problems using our own `re_log` crate (which is currently a wrapper around [`tracing`](https://crates.io/crates/tracing/)).

* An error should never happen in silence.
* Validate code invariants using `assert!` or `debug_assert!`.
* Validate user data and return errors using [`thiserror`](https://crates.io/crates/thiserror).
* Attach context to errors as they bubble up the stack using [`anyhow`](https://crates.io/crates/anyhow).
* Log errors using `re_log::error!` or `re_log::error_once!`.
* If a problem is recoverable, use `re_log::warn!` or `re_log::warn_once!`.
* If an event is of interest to the user, log it using `re_log::info!` or `re_log::info_once!`.
* The code should only panic if there is a bug in the code.
* Never ignore an error: either pass it on, or log it.
* Handle each error exactly once. If you log it, don't pass it on. If you pass it on, don't log it.

### Log levels

The log is for several distinct users:
* The application user
* The application programmer
* The library user
* The library programmer

We are all sharing the same log stream, so we must cooperate carefully.

#### `ERROR`
This is for _unrecoverable_ problems. The application or library couldn't complete an operation.

Libraries should ideally not log `ERROR`, but instead return `Err` in a `Result`, but there are rare cases where returning a `Result` isn't possible (e.g. then doing an operation in a background task).

Application can "handle" `Err`ors by logging them as `ERROR` (perhaps in addition to showing a popup, if this is a GUI app).

#### `WARNING`
This is for _recoverable_ problems. The operation completed, but couldn't do exactly what it was instructed to do.

Sometimes an `Err` is handled by logging it as `WARNING` and then running some fallback code.

#### `INFO`
This is the default verbosity level. This should mostly be used _only by application code_ to write interesting and rare things to the application user. For instance, you may perhaps log that a file was saved to specific path, or where the default configuration was read from. These things lets application users understand what the application is doing, and debug their use of the application.

#### `DEBUG`
This is a level you opt-in to to debug either an application or a library. These are logged when high-level operations are performed (e.g. texture creation). If it is likely going to be logged each frame, move it to `TRACE` instead.

#### `TRACE`
This is the last-resort log level, and mostly for debugging libraries or the use of libraries. Here any and all spam goes, logging low-level operations.

The distinction between `DEBUG` and `TRACE` is the least clear. Here we use a rule of thumb: if it generates a lot of continuous logging (e.g. each frame), it should go to `TRACE`.


### Libraries
We use [`thiserror`](https://crates.io/crates/thiserror) for errors in our libraries, and [`anyhow`](https://crates.io/crates/anyhow) for type-erased errors in applications.

For faster hashing, we use [`ahash`](https://crates.io/crates/ahash) (`ahash::HashMap`, …).

When the hashmap key is high-entropy we use [`nohash-hasher`](https://crates.io/crates/nohash-hasher) (`nohash_hasher::IntMap`).

### Style
We follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/about.html).

We use `rust fmt` with default settings.

We have blank lines before functions, types, `impl` blocks, and docstrings.

We format comments `// Like this`, and `//not like this`.

When importing a `trait` to use it's trait methods, do this: `use Trait as _;`. That lets the reader know why you imported it, even though it seems unused.

When intentionally ignoring a `Result`, prefer `foo().ok();` over `let _ = foo();`. The former shows what is happening, and will fail to compile if `foo`:s return type ever changes.

### `TODO`:s
When you must remember to do something before merging a PR, write `TODO` or `FIXME` in any file. The CI will not be green until you either remove them or rewrite them as `TODO(yourname)`.

You can also use the `todo()!` macro during development, but again it won't pass CI until you rewrite it as `todo!("more details")`. Of course, we should try to avoid `todo!` macros in our code.


### Misc
Use debug-formatting (`{:?}`) when logging strings in logs and error messages. This will surround the string with quotes and escape newlines, tabs, etc. For instance: `re_log::warn!("Unknown key: {key:?}");`.

Use `re_error::format(err)` when displaying an error.

### Naming
When in doubt, be explicit. BAD: `id`. GOOD: `msg_id`.

Be terse when it doesn't hurt readability. BAD: `message_identifier`. GOOD: `msg_id`.

Avoid negations in names. A lot of people struggle with double negations, so things like `non_blocking = false` and `if !non_blocking { … }` can become a source of confusion and will slow down most readers. So prefer `connected` over `disconnected`, `initialized` over `uninitialized` etc.

For UI functions (functions taking an `&mut egui::Ui` argument), we use the name `ui` or `_ui` suffix, e.g. `blueprint_ui(…)` or `blueprint.ui(…)`.

#### Spaces
Points, vectors, rays etc all live in different _spaces_. Whenever there is room for ambiguity, we explicitly state which space something is in, e.g. with `ray_in_world`.

Here are some of our standard spaces:

* `ui`: coordinate system used by `egui`, measured in logical pixels ("points"), with origin in the top left
* `image`: image pixel coordinates, possibly with an added `z=depth`
* `space`: a user-defined space where they log stuff into
* `world`: the common coordinate system of a 3D scene, usually same as `space`
* `view`: X=right, Y=down, Z=back, origin = center of screen

#### Matrices
We use column vectors, which means matrix multiplication is done as `M * v`, i.e. we read all matrix/vector operations right-to-left. We therefore name all transform matrices as `foo_from_bar`, for instance:

```rust
let point_in_world = world_from_view * point_in_view;
```

This means the name of the space matches up nicely, e.g.:

```rust
let projection_from_object = projection_from_view * view_from_world * world_from_object;
```

See <https://www.sebastiansylvan.com/post/matrix_naming_convention/> for motivation.

For consistency, we use the same naming convention for other non-matrix transforms too. For instance, functions: `let screen = screen_from_world(world);`.

#### Vectors vs points
Vectors are directions with magnitudes. Points are positions.


## Improving compile times

As of today, we link everything statically in both debug and release builds, which makes custom linkers and split debuginfo the two most impactful tools we have at our disposal in order to improve compile times.

These tools can configured through your `Cargo` configuration, available at `$HOME/.cargo/config.toml`.

### macOS

On macOS, use the [zld](https://github.com/michaeleisel/zld) linker and keep debuginfo in a single separate file.

Pre-requisites:
- Install [zld](https://github.com/michaeleisel/zld): `brew install michaeleisel/zld/zld`.

`config.toml` (x64):
```toml
[target.x86_64-apple-darwin]
rustflags = [
    "-C",
    "link-arg=-fuse-ld=/usr/local/bin/zld",
    "-C",
    "split-debuginfo=packed",
]
```

`config.toml` (M1):
```toml
[target.aarch64-apple-darwin]
rustflags = [
    "-C",
    "link-arg=-fuse-ld=/opt/homebrew/bin/zld",
    "-C",
    "split-debuginfo=packed",
]
```

### Linux

On Linux, use the [mold](https://github.com/rui314/mold) linker and keep DWARF debuginfo in separate files.

Pre-requisites:
- Install [mold](https://github.com/rui314/mold) through your package manager.

`config.toml`:
```toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = [
    "-C",
    "link-arg=-fuse-ld=/usr/bin/mold",
    "-C",
    "split-debuginfo=unpacked",
]
```

### Windows

On Windows, use LLVM's `lld` linker and keep debuginfo in a single separate file.

Pre-requisites:
- Install `lld`:
```
cargo install -f cargo-binutils
rustup component add llvm-tools-preview
```

`config.toml`:
```toml
[target.x86_64-pc-windows-msvc]
linker = "rust-lld.exe"
rustflags = [
    "-C",
    "split-debuginfo=packed",
]
```
