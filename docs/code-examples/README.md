# API Examples

These examples showcase common usage of each individual Rerun `Archetype`s.

Most of these example are automatically used as docstrings for the `Archetype` APIs in their respective SDKs.

## Usage

You can run each example individually using the following:

- **Python**: `python <example_name>.py`, e.g. `python point3d_random.py`.
- **Rust**: `cargo r -p code_examples --bin <example_name`, e.g.
  `cargo r -p code_examples --bin point3d_random`.
- **C++**:
  - `./docs/code-examples/build_all.sh` to compile all examples
  - start a Rerun Viewer listening on the default port: `rerun`
  - `./build/docs/code-examples/doc_example_<example_name>` to run, e.g.  `./build/docs/code-examples/doc_example_point3d_random`

## Roundtrips

All API examples support cross-language roundtrip tests.

These tests check that A) all of our SDKs yield the exact same data when used the same way and B) act as regression tests, relying on the fact that it is extremely unlikely that all supported languages break in the exact same way at the exact same time.

See `./docs/code-examples/roundtrips.py --help`.
