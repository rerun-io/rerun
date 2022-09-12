# Contributing to Rerun
This is written for anyone who wants to contribute to the Rerun repository.

Rerun is an open core company, and this repository is dual-licensed under MIT and APACHE. However, this repository is NOT YET open source, but IT WILL BE. Therefore we ask you to avoid making public clones of this repository, but in other respects treat it as any other open source GitHub project.

## Setup
First up, you need to install the Rust toolchain: <https://rustup.rs/>. Then run `./scripts/setup_dev.sh`.

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

To check everything in one go, run `./scripts/check.sh`. `check.sh` should ideally check approximately the same things as our CI.

### Optional
You can use [bacon](https://github.com/Canop/bacon) to automatically check your code on each save. For instance, running just `bacon` will re-run `cargo cranky` each time you change a rust file. See `bacon.toml` for more.

### Other
You can view higher log levels with `export RUST_LOG=debug` or `export RUST_LOG=trace`.


## Rust code

### Error handling and logging
* An error should never happen in silence.
* Validate code invariants using `assert!`.
* Validate user data and return errors using [`thiserror`](https://crates.io/crates/thiserror).
* Attach context to errors as they bubble up the stack using [`anyhow`](https://crates.io/crates/anyhow).
* Log errors using `log::error!` or `log_once::error_once!`.
* If a problem is recoverable, use `tracing::warn!` or `log_once::warn_once!`.
* If an event is of interest to the user, log it using `tracing::info!` or `log_once::info_once!`.
* The code should only panic if there is a bug in the code.
* Never ignore an error: either pass it on, or log it.
* Handle each error exactly once. If you log it, don't pass it on. If you pass it on, don't log it.


### Libraries
We use [`tracing`](https://crates.io/crates/tracing/) for logging.

We use [`thiserror`](https://crates.io/crates/thiserror) for errors in our libraries, and [`anyhow`](https://crates.io/crates/anyhow) for type-erased errors in applications.

### Style
We follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/about.html).

We use `rust fmt` with default settings.

We have blank lines before functions, types, `impl` blocks, and docstrings.

We format comments `// Like this`, and `//not like this`.

When importing a `trait` to use it's trait methods, do this: `use Trait as _;`. That lets the reader know why you imported it, even though it seems unused.

When intentionally ignoring a `Result`, prefer `foo().ok();` over `let _ = foo();`. The former shows what is happening, and will fail to compile if `foo`:s return type ever changes.

### Misc
Use debug-formatting (`{:?}`) when logging strings in logs and error messages. This will surround the string with quotes and escape newlines, tabs, etc. For instance: `tracing::warn!("Unknown key: {key:?}");`.

Use `re_error::format(err)` when displaying an error.

### Naming
When in doubt, be explicit. BAD: `id`. GOOD: `msg_id`.

Be terse when it doesn't hurt readability. BAD: `message_identifier`. GOOD: `msg_id`.

Avoid negations in names. A lot of people struggle with double negations, so things like `non_blocking = false` and `if !non_blocking { … }` can become a source of confusion and will slow down most readers. So prefer `connected` over `disconnected`, `initialized` over `uninitialized` etc.

#### Spaces
Points, vectors, rays etc all live in different _spaces_. Whenever there is room for ambiguity, we explicitly state which space something is in, e.g. with `ray_in_world`.

Here are some of our standard spaces:

* `ui`: coordinate system used by `egui`, measured in logical pixels ("points"), with origin in the top left
* `image`: image pixel coordinates, possibly with an added z=depth
* `space`: a user-defined space where they log stuff into
* `world`: the common coordinate system of a 3D screen, usually same as `space`
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
