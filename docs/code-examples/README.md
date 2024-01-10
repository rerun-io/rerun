# API Examples

These examples showcase common usage of each individual Rerun `Archetype`s.

Most of these examples are automatically used as docstrings for the `Archetype` APIs in their respective SDKs, as well as the [Archetypes](https://www.rerun.io/docs/reference/types) section of the high-level documentation.

## Usage

You can run each example individually using the following:

- **C++**:
  - `pixi run cpp-build-doc-examples` to compile all examples
  - `./build/docs/code-examples/all/<example_name>` to run, e.g. `./build/docs/code-examples/all/point3d_random`
- **Python**: `python <example_name>.py`, e.g. `python point3d_random.py`.
- **Rust**: `cargo run -p code_examples -- <example_name`, e.g. `cargo run -p code_examples -- point3d_random`.

## Roundtrips

All API examples support cross-language roundtrip tests: i.e. we execute the same logging commands from all 3 SDKs, save the results to distinct rrd files, and finally compare these rrd files.
These tests are then automatically run by the CI, which will loudly complain if the resulting rrd files don't match.

These tests check that A) all of our SDKs yield the exact same data when used the same way and B) act as regression tests, relying on the fact that it is extremely unlikely that all supported languages break in the exact same way at the exact same time.

### Usage

To run the roundtrip tests, check out `./docs/code-examples/roundtrips.py --help`.
`./docs/code-examples/roundtrips.py` is a valid invocation that will build all 3 SDKs and run all tests for all of them.

### Implementing new tests

Just pick a name for your test, and look at existing examples to get started. The `app_id` must be `rerun_example_<test_name>`.

The roundtrip process is driven by file names, so make sure all 3 tests use the same name: `<test_name>.rs`, `<test_name>.cpp`, `<test_name>.py`.
