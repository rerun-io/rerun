# API examples

These examples showcase common usage of each individual Rerun `Archetype`s.

Most of these examples are automatically used as docstrings for the `Archetype` APIs in their respective SDKs, as well as the [Archetypes](https://www.rerun.io/docs/reference/types) section of the high-level documentation.

## Usage

You can run each example individually using the following:

- **C++**:
  - `pixi run -e cpp cpp-build-snippets` to compile all examples
  - `./build/debug/docs/snippets/all/<example_name>` to run, e.g. `./build/debug/docs/snippets/all/point3d_random`
- **Python**: `pixi run py-build && pixi run -e py python <example_name>.py`, e.g. `pixi run -e py python point3d_random.py`.
- **Rust**: `cargo run -p snippets -- <example_name> [args]`, e.g. `cargo run -p snippets -- point3d_random`.

## Comparison test

The script `compare_snippet_output.py` execute the same logging commands from all 3 SDKs, save the results to distinct rrd files, and finally compare these rrd files.
These tests are then automatically run by the CI, which will loudly complain if the resulting rrd files don't match.

These tests check that A) all of our SDKs yield the exact same data when used the same way and B) act as regression tests, relying on the fact that it is extremely unlikely that all supported languages break in the exact same way at the exact same time.

### Usage

To run the comparison tests, check out `pixi run -e py docs/snippets/compare_snippet_output.py --help`.
`pixi run -e py docs/snippets/compare_snippet_output.py` is a valid invocation that will build all 3 SDKs and run all tests for all of them.

### Implementing new tests

Just pick a name for your test, and look at existing examples to get started. The `app_id` must be `rerun_example_<test_name>`.

The comparison process is driven by file names, so make sure all 3 tests use the same name: `<test_name>.rs`, `<test_name>.cpp`, `<test_name>.py`.
