# Documentation snippets

Small, self-contained examples in `all/`, organized by category (`archetypes/`, `howto/`, `tutorials/`, `views/`, etc.). Most snippets have `.py`, `.rs`, and `.cpp` versions with the same base name, and are automatically used as docstrings for the `Archetype` APIs and the [Archetypes](https://www.rerun.io/docs/reference/types) documentation.

## Running snippets

Rust and C++ snippets compile into a single dispatcher binary that takes the snippet name (without path/extension) as first argument.

- **C++**: `pixi run -e cpp cpp-build-snippets`, then `./build/debug/docs/snippets/all/<name>`
- **Python**: `pixi run py-build && pixi run uvpy <name>.py`
- **Rust**: `cargo run -p snippets -- <name> [args]`

## Build system

Both `build.rs` (Rust) and `CMakeLists.txt` (C++) auto-copy snippet sources from `all/`, rename `main` to a per-snippet function, and generate a dispatcher. Don't edit files in `src/snippets/` directly.

## Finding existing snippets

`INDEX.md` is an auto-generated index (by codegen) mapping features/archetypes to snippets with per-language links. Check it before writing new snippets.

## Snippet configuration

`snippets.toml` controls snippet testing and documentation indexing. See the comments in that file for details.

## Comparison tests

`compare_snippet_output.py` runs the same logging commands from all 3 SDKs, saves to distinct rrd files, and compares them. CI runs these automatically.

These tests verify:
- All SDKs yield identical data when used the same way
- Act as regression tests (extremely unlikely all languages break identically)

### Running comparison tests

- `pixi run uvpy docs/snippets/compare_snippet_output.py --help` for options
- `pixi run uvpy docs/snippets/compare_snippet_output.py` builds all 3 SDKs and runs all tests

### Implementing new tests

- Pick a name, look at existing examples to get started
- Use the same name across languages: `<name>.rs`, `<name>.cpp`, `<name>.py`
- Set `app_id` to `rerun_example_<name>`
