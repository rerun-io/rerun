# Rerun code style

## See also
* [`ARCHITECTURE.md`](ARCHITECTURE.md)
* [`BUILD.md`](BUILD.md)
* [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
* [`CONTRIBUTING.md`](CONTRIBUTING.md)
* [`RELEASES.md`](RELEASES.md)


## Languages
We prefer Rust.

We have a bunch of Bash and Python scripts that [we want to replace with Rust](https://github.com/rerun-io/rerun/issues/3349).

For configs we like JSON and TOML, and [dislike YAML](https://ruudvanasseldonk.com/2023/01/11/the-yaml-document-from-hell).


## Rust code

### Avoid `unsafe`
`unsafe` code should be only used when necessary, and should be carefully scrutinized during PR reviews.

### Avoid `unwrap`, `expect` etc.
The code should never panic or crash, which means that any instance of `unwrap` or `expect` is a potential time-bomb. Even if you structured your code to make them impossible, any reader will have to read the code very carefully to prove to themselves that an `unwrap` won't panic. Often you can instead rewrite your code so as to avoid it. The same goes for indexing into a slice (which will panic on out-of-bounds) - it is often preferable to use `.get()`.

For instance:

``` rust
let first = if vec.is_empty() {
    return;
} else {
    vec[0]
};
```
can be better written as:

``` rust
let Some(first) = vec.get(0) else {
    return;
};
```

### Iterators
Be careful when iterating over `HashSet`s and `HashMap`s, as the order is non-deterministic.
Whenever you return a list or an iterator, sort it first.
If you don't want to sort it for performance reasons, you MUST put `unsorted` in the  name as a warning.

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
* Put any sensitive data (like URLs, file paths etc) LAST in the error message, so that users can send us the first half and omit the sensitive half.
* Prefer returning an error to the caller over logging it and carrying on.
  Do fallible setup (binding a socket, opening a device) before spawning a thread, so that the failure can be returned.
* Don't `.map_err(|err| err.to_string())` unless the function returns `Result<_, String>` — it throws away the error type.
* Don't drop an error with `.map_err(|_err| …)` unless you log it or wrap it first.
* Use `warn_once!`/`error_once!` for messages that would otherwise repeat every frame.
* Invalid configuration (env vars, CLI args, config files) is an error, not a silent fallback to the default.

Strive to encode code invariants and contracts in the type system as much as possible. So if a vector cannot be empty, consider using [`vec1`](https://crates.io/crates/vec1). [Parse, don’t validate](https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/).

Prefer strong types over primitive ones:
* `url::Url` and `PathBuf` over `String`.
* A newtype (`struct StreamId(u64)`) over a bare integer or a type alias.
* An enum over a `bool` field or parameter (`enum Screen { Setup, Chat }` over `show_setup: bool`).
* `Option<Result<(), Error>>` over `Option<bool>`.
* A named struct (or `Rangef`, `Span`, …) over a tuple.
* `Option<NonZeroU64>` over a `u64` where `0` means "disabled".

If a path or file marker could silently go stale, add a `debug_assert!` or a test that checks it.

Some contracts cannot be enforced using the type system. In those cases you should explicitly enforce them using `assert` (self-documenting code) and in documentation (if it is part of a public API).

### Log levels

The log is for several distinct users:
* The application user
* The application programmer
* The library user
* The library programmer

We are all sharing the same log stream, so we must cooperate carefully.

The Rerun viewer will show log messages at `INFO`, `WARNING` and `ERROR` to the user as a toast notifications.

#### `ERROR`
This is for _unrecoverable_ problems. The application or library couldn't complete an operation.

Libraries should ideally not log `ERROR`, but instead return `Err` in a `Result`, but there are rare cases where returning a `Result` isn't possible (e.g. then doing an operation in a background task).

Application can "handle" `Err`ors by logging them as `ERROR` (perhaps in addition to showing a popup, if this is a GUI app).

Use this log level whenever some data is lost, even if you continue processing other data.

Examples: failing to write a file, failing to read parts of a file.

#### `WARNING`
This is for _recoverable_ problems. The operation completed, but couldn't do exactly what it was instructed to do.

Sometimes an `Err` is handled by logging it as `WARNING` and then running some fallback code.

Warnings are also used for thing that _may_ be an error, but it could be intended (e.g. dropping a sink before flushing it).

Examples: usage of deprecated functions, slow paths, misuse of our APIs, lossy data conversion.

If data is lost, it is an error and NOT a warning.

#### `INFO`
This is the default verbosity level. This should mostly be used _only by application code_ to write interesting and rare things to the application user. For instance, you may perhaps log that a file was saved to specific path, or where the default configuration was read from. These things lets application users understand what the application is doing, and debug their use of the application.

#### `DEBUG`
This is a level you opt-in to to debug either an application or a library. These are logged when high-level operations are performed (e.g. texture creation). If it is likely going to be logged each frame, move it to `TRACE` instead.

#### `TRACE`
This is the last-resort log level, and mostly for debugging libraries or the use of libraries. Here any and all spam goes, logging low-level operations.

The distinction between `DEBUG` and `TRACE` is the least clear. Here we use a rule of thumb: if it generates a lot of continuous logging (e.g. each frame), it should go to `TRACE`.


### Warning reporter pattern
For reporting warnings (or partial-failures) up the call-stack, we like the _reporter pattern_:

```rs
struct WarningReporter {
    reports: Mutex<Vec<Warning>>,
}

pub fn thing_that_can_produce_warnings(reporter: &WarningReporter, other_paramets: …) -> Result<…> {}
```

The important parts of this pattern is:
* Accumulate warnings and then continue
* Interior mutability, so we can share the reporter with child threads
* Structured warnings (more than just a String!)

We use this for _partial failures_, when something went wrong but we don't want to abort, but instead continue with best-effort.

We prefer this pattern complex return types (`(Vec<Warning>, Object)`), because the reporter pattern is often a lot less syntactically noisy in Rust.
It is also easy to ignore part of a return-type, but it is harder to ignore an extra parameter. Thus we force ourselves to handle warnings.

This allows code like this:

```rs
fn some_panel_ui(ctx: &ViewerContext, ui: &mut Ui) {
    let reporter = WarningReporter::default();
    let object = do_some_query(&reporter, …)?;
    object.ui(ui);
    if reporter.is_missing_chunks() {
        ui.loading_indicator("Doing query");
    }
    if !reporter.warnings().is_empty() {
        warnings_ui(reporter.warnings());
    }
}
````

### Libraries
We use [`thiserror`](https://crates.io/crates/thiserror) for errors in our libraries, and [`anyhow`](https://crates.io/crates/anyhow) for type-erased errors in applications.
Don't add `anyhow` to a library crate, not even for a one-off error: add a variant to the crate's error type instead.

For faster hashing, we use [`ahash`](https://crates.io/crates/ahash) (`ahash::HashMap`, …).

When the hashmap key is high-entropy we use [`nohash-hasher`](https://crates.io/crates/nohash-hasher) (`nohash_hasher::IntMap` and `IntSet`).
This includes any key that is itself a hash, e.g. `EntityPath`, `ComponentDescriptor`, `InternedString`, and other types that implement `nohash_hasher::IsEnabled`.

Use `re_format` to format numbers, durations, and sizes for humans, e.g. `format_uint`, `format_f64`, `format_bytes`, and `DurationFormatOptions`, rather than `format!("{x}")`.
That way we get the same thousands separators, minus sign, and units everywhere.

### Style
We follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/about.html).

We use `rust fmt` with default settings.

We have blank lines before functions, types, `impl` blocks, and docstrings.

We format comments `// Like this`, and `//not like this`.

When importing a `trait` to use its trait methods, do this: `use Trait as _;`. That lets the reader know why you imported it, even though it seems unused.

When intentionally ignoring a `Result`, prefer `foo().ok();` over `let _ = foo();`. The former shows what is happening, and will fail to compile if `foo`:s return type ever changes.

We group and order imports (`use` statements) by `std`, other crates, and lastly own `crate` and `super`. This corresponds to [`StdExternalCrate`](https://rust-lang.github.io/rustfmt/?version=v1.8.0&search=group#StdExternalCrate%5C%3A).

We group our `use` statements by module, e.g. `crate_name::module::{a, b, c}`. This is a compromise, being rather terse while still avoiding excessive merge conflicts. See [the cargofmt docs](https://rust-lang.github.io/rustfmt/?version=v1.8.0&search=group#Module%5C%3A) for details.

Both of these are nightly-only rustfmt options (`group_imports` and `imports_granularity`), so the stable `cargo fmt` that CI runs does not enforce them.
We re-apply them now and then with the `nightly-fmt` skill ([`.claude/skills/nightly-fmt/SKILL.md`](../.claude/skills/nightly-fmt/SKILL.md)), run as `nightly-fmt.sh --imports`.

Use the destructor syntax (`let Self { a, b, c} = self;`) whenever you're accessing most of (or all) of the fields of a struct.
This also makes the compiler tell you when a new field is added that you may have forgotten about.

If you use a type or function more than twice in a file, add a `use` for it at the top of the file instead of spelling out its full path each time.

Keep hand-written lists sorted alphabetically, unless the order carries meaning.
This includes enum variants, match arms, Cargo features, `clippy.toml` entries, `.gitattributes` lines, and tables in docs.

Prefer `cfg_select!` over pairs of `#[cfg(…)]`/`#[cfg(not(…))]` blocks.
For UI, prefer a runtime check (e.g. a disabled button on web) over compile-time `#[cfg]`.

Add `re_tracing::profile_function!()` (or `profile_scope!`) to functions that may be slow, e.g. I/O, encoding and decoding, opening or closing devices, and large loops.

Don't wrap code in a closure that is called immediately, and don't extract a helper that is only called once unless it names a concept.

Avoid catch-all `_ =>` arms when matching on our own enums, so that the compiler flags every `match` that needs updating when a variant is added.

Every new timeout, size limit, or threshold needs a reason: say where the number comes from, or reuse an existing constant.
Refer to metadata keys and column names through their constants, never through a string literal.

### API design
* Prefer constructors and associated functions over free functions.
* Avoid global state (`static FOO: LazyLock<…>`); let the caller own it.
* Instead of `fn should_do_x(x) -> bool` + `fn do_x(x)`, write a single `fn do_x(x) -> Option<…>`.
* Keep items private by default, and make only the intended API `pub`.
  Don't make a field `pub` if it also has a setter.
* Keep `lib.rs` and `mod.rs` short: mostly `mod` declarations and `pub use` statements that list what the crate exports.
  Small items like constants are fine there too; move functions and types with real logic into their own modules.
* Don't add traits, public methods, or hooks to production code only so that tests can use them.
* Don't add code "for later", or backwards compatibility that nothing needs.
* Before writing a helper, look for an existing one in this repo or in a dependency we maintain (egui, emath, quiver, …).
  If it is missing there, add it upstream instead of working around it locally.

### Comments and docstrings
Every crate-level public item, and every public field such structs, should have a docstring.
Document behavior, not implementation: what it does, units, defaults, and what happens in edge cases (missing, empty, already exists, …).
Explain domain terms for readers who are not experts in that domain.
If something has several names (e.g. `application_id` is the dataset id), list them.

Start a multi-line docstring with a one-line summary, followed by an empty line.
Put the docstring before any `#[derive(…)]` attributes.

Comments must add durable value: explain non-obvious invariants, contracts, performance constraints, or caller obligations.
Don't write comments that justify a change to the reviewer, narrate what the next line does, or describe history ("no longer", "deliberately", "being phased out").
A comment describes the code it is attached to: don't describe what other code does ("see the module docs", "same as the X branch"), or what may happen in the future.
Document a fact once, where it is true, and link to it from elsewhere.

A wrong comment is worse than no comment: after changing or renaming code, check that the comments around it are still true.

### `TODO`:s
When you must remember to do something before merging a PR, write `TODO` or `FIXME` in any file. The CI will not be green until you either remove them or rewrite them as `TODO(yourname)`.

For work that is left for a later PR, prefer linking an issue, e.g. `TODO(rerun-io/rerun#1234)` or `TODO(RR-1234)`, so it doesn't get lost.

You can also use the `todo()!` macro during development, but again it won't pass CI until you rewrite it as `todo!("more details")`. Of course, we should try to avoid `todo!` macros in our code.


### Misc
Use debug-formatting (`{:?}`) when logging strings in logs and error messages. This will surround the string with quotes and escape newlines, tabs, etc. For instance: `re_log::warn!("Unknown key: {key:?}");`.

Use `{:#}` or `re_error::format(err)` when displaying an error - NOT `Debug`/`{:?}`.

Derive egui ids from the parent `Ui` with `ui.make_persistent_id(…)`, so that two instances of the same widget don't collide.
Hash a tuple rather than building an id with `format!`: `ui.make_persistent_id(("plot_legend", view_id))`.

We make extensive use of snapshot testing. To work around non-deterministic values, such as TUIDs (time-prefixed unique IDs), many types (should) offer `std::fmt::Display` implementations with redactions that can be access via an overloaded `-` formatting option:

```rs
println!("{:-}, value"); // The `-` option stands for redaction.
```

Look for `f.sign_minus()` in the code for where we handle this.

## Python
Prefer kw-args (key-word arguments) for non-obvious parameters, especially when there are many of them.

* Bad: `def serve(ip: str, port: int = 80, token: str | None = None, timeout_sec: int = 8)`
* Better: `def serve(ip: str, *, port: int = 80, token: str | None = None, timeout_sec: int = 8)`
* Best: `def serve(*, ip: str, port: int = 80, token: str | None = None, timeout_sec: int = 8)`

This forces the use of kw-args for everything following the `*`.

kw-args have two big benefits:

First, they make future API changes a lot easier. We can add or remove arguments without breaking the API (just log deprecation notices).

Secondly, named arguments makes the caller code a lot more readable.

Usually, we do NOT use kw-args for single-parameter functions, nor for functions where the first (or all) parameters are obvious from the caller. For instance, `def load_file(path: str) -> bytes`

## C++
We use `clang-format` to enforce most style choices (see [`.clang-format`](.clang-format)).

### Initialization
Always use `const` unless you plan on mutating it, with the exception of function parameters (because that is just too much noise).

We use `const auto x = …` for declaration because that gives symmetric code for normal constructors and static constructors:

```C++
const auto foo = SomeClass{…};
const auto bar = SomeClass::new_xyzw(…);
```

We prefer `{}` for constructors (`Foo{…}` instead of `Foo(…)`), though there are exceptions (`std::vector{2, 3}` is different from `std::vector(2, 3)`).

Prefer `using Type = …;` over `typedef … Type;`.

### Members
We prefix _private_ member variables with a `_`:

```C++
class Thing {
  public:
    …

    void set_value(uint32_t value) {
        _value = value;
    }

  private:
    uint32_t _value;
}
```

Public member variables has no prefix.
When necessary use a `_` suffix on parameter names to avoid name conflicts:

```C++
struct Thing {
    uint32_t value;

    void set_value(uint32_t value_) {
        value = value_;
    }
}
```

### Constructors and builder pattern
We use C++ constructors when it is unambiguous, but prefer _named static constructors_ otherwise.
Like Rust, we use the `from_` prefix for static constructors, and the `with_` prefix for builder methods.

```C++
class Rect {
    // We can't just overload normal constructors for these:
    static Rect from_min_max(Vec2 min, Vec2 max) { … }
    static Rect from_center_size(Vec2 center, Vec2 size) { … }

    Rect with_color(Color color) && {
        _color = color;
        return std::move(*this); // `*this` is always an lvalue, so we have to move it to avoid a copy.
    }
}
```

### Constants & Enums

Constants & enum values have PascalCase names.

When possible, use `constexpr` for (global & struct/class scoped) constants.

### String handling
Whenever possible we use `std::string_view` to pass strings.

To accommodate for this and other languages, strings on the C interface are almost never expected to be null-terminated and are always passed along with a byte length using `rr_string`.


### Misc
We don't add `inline` before class/struct member functions if they are inlined in the class/struct definition.

Preprocessor directives/macros are usually prefixed with `RR_`

Include what you use: if you use `std::vector`, then include `<vector>` - don't depend on a transitive include.

We prefer the "data, length" parameter order, e.g. `void foo(const void* data, size_t len)` or `void image(const f32* data, Resolution resolution)`.


## Naming
We prefer `snake_case` to `kebab-case` for most things (e.g. crate names, crate features, …). `snake_case` is a valid identifier in almost any programming language, while `kebab-case` is not. This means one can use the same `snake_case` identifier everywhere, and not think about whether it needs to be written as `snake_case` in some circumstances.

When in doubt, be explicit. BAD: `id`. GOOD: `msg_id`.

Be terse when it doesn't hurt readability. BAD: `message_identifier`. GOOD: `msg_id`.

Avoid negations in names. A lot of people struggle with double negations, so things like `non_blocking = false` and `if !non_blocking { … }` can become a source of confusion and will slow down most readers. So prefer `connected` over `disconnected`, `initialized` over `uninitialized` etc.

Name things for what they are, not for what they are used for.

Don't repeat the type in the name: `age`, not `age_integer`.

Use one name for one concept, everywhere.

For UI functions (functions taking an `&mut egui::Ui` argument), we use the name `ui` or `_ui` suffix, e.g. `blueprint_ui(…)` or `blueprint.ui(…)`.

### Be over-explicit in stringly typed situations
In weak/stringly typed situations, be extra careful. This includes Python, Bash, and CLI args.

Avoid vague names like "address". Prefer one of:

* `ip`
* `ip_port`
* `url`
* `email`
* …


### Units
* When in doubt, be explicit (`duration_secs: f32` is better than `duration: f32`)
* All things being equal, prefer SI base units (seconds over milliseconds, Hz over RPM, etc)
* When precision matters, prefer `nanos: i64` over `secs: f64`
* Store angles in radians (but you may print/display them as degrees)

Follow the conventions set by Rust stdlib:
* `secs` instead of `seconds`
* `millis` instead of `ms` or `milliseconds`
* `micros` instead of `us` or `microseconds`
* `nanos` instead of `ns` or `nanoseconds`


### Spaces
Points, vectors, rays etc all live in different _spaces_. Whenever there is room for ambiguity, we explicitly state which space something is in, e.g. with `ray_in_world`.

Here are some of our standard spaces:

* `ui`: coordinate system used by `egui`, measured in logical pixels ("points"), with origin in the top left
* `image`: image pixel coordinates, possibly with an added `z=depth`
* `space`: a user-defined space where they log stuff into
* `world`: the common coordinate system of a 3D scene, usually same as `space`
* `view`: X=right, Y=down, Z=back, origin = center of screen

### Matrices
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

### Vectors vs points
Vectors are directions with magnitudes. Points are positions.
