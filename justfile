# Install just: https://github.com/casey/just
#
# Then run `just --list` to see the available commands

export RUSTDOCFLAGS := "--deny warnings --deny rustdoc::missing_crate_level_docs"
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

default:
  @just --list


### Common
# Format all of our code
format: cpp-format toml-format py-format
    cargo fmt --all

# Lint all of our code
lint: toml-lint py-lint rs-lint

# Run the fast versions of our linters
fast-lint *ARGS:
    pixi run fast-lint {{ARGS}}

### C and C++

# Clear the C++ build directories
cpp-clean:
    rm -rf build CMakeCache.txt CMakeFiles

cpp-format:
    #!/usr/bin/env bash
    fd --extension h --exec clang-format -i
    fd --extension hpp --exec clang-format -i
    fd --extension c --exec clang-format -i
    fd --extension cpp --exec clang-format -i

# Build our C++ SDK, and all our tests and examples
cpp-build-all:
    pixi run cpp-build-all

# Build our C++ SDK and tests
cpp-build:
    pixi run cpp-build-all

# Build all our C++ examples.
cpp-build-examples:
    pixi run cpp-build-examples

# Build all our C++ api doc examples.
cpp-build-doc-examples:
    pixi run cpp-build-doc-examples

# Run our C++ tests
cpp-test:
    pixi run cpp-test

cpp-plot-dashboard *ARGS:
    pixi run cpp-plot-dashboard {{ARGS}}


### Python

py_folders := "docs/code-examples examples rerun_py scripts tests"

# Set up a Pythonvirtual environment for development
py-dev-env:
    #!/usr/bin/env bash
    echo "Setting up Python virtual environment in venv"
    set -euxo pipefail
    python3 -m venv venv
    venv/bin/pip install --upgrade pip
    venv/bin/pip install -r scripts/requirements-dev.txt
    echo "Do 'source venv/bin/activate' to use the virtual environment!"

# Run all examples with the specified args
py-run-all *ARGS:
    python3 "scripts/run_all.py" {{ARGS}}

# Run all examples in the native viewer
py-run-all-native: py-run-all

# Run all examples in the web viewer
py-run-all-web:
    just py-run-all --web

# Run all examples, save them to disk as rrd, then view them natively
py-run-all-rrd *ARGS:
    just py-run-all --save {{ARGS}}

# Run all examples with all supported Python versions (through nox)
py-run-all-allpy *ARGS:
    nox -s run_all -- {{ARGS}}

# Build and install the package into the venv
py-build *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    unset CONDA_PREFIX && \
        maturin develop \
            --manifest-path rerun_py/Cargo.toml \
            --extras="tests" \
            {{ARGS}}

# Run autoformatting
py-format:
    #!/usr/bin/env bash
    set -euo pipefail
    # NOTE: we need both `ruff check --fix` and `ruff format` in that order: https://twitter.com/charliermarsh/status/1717229721954799727
    ruff check --fix --config rerun_py/pyproject.toml {{py_folders}}
    ruff format --config rerun_py/pyproject.toml {{py_folders}}
    blackdoc {{py_folders}} # Format code examples in docstring. Hopefully `ruff` can do this soon: https://github.com/astral-sh/ruff/issues/7146

# Check that all the requirements.txt files for all the examples are correct
py-requirements:
    #!/usr/bin/env bash
    set -euo pipefail
    find examples/python/ -name main.py | xargs -I _ sh -c 'cd $(dirname _) && echo $(pwd) && pip-missing-reqs . || exit 255'

# Run linting
py-lint:
    #!/usr/bin/env bash
    set -euxo pipefail
    ruff check --config rerun_py/pyproject.toml {{py_folders}}
    ruff format --check --config rerun_py/pyproject.toml {{py_folders}}
    blackdoc --check {{py_folders}}
    mypy --install-types --non-interactive --no-warn-unused-ignore

# Run fast unittests
py-test:
    python -m pytest -vv -c rerun_py/pyproject.toml rerun_py/tests/unit/

# Run tests on all supported Python versions (through nox)
py-test-allpy:
    nox -s tests

# Run all Python benchmarks
py-bench *ARGS:
    python -m pytest -c rerun_py/pyproject.toml --benchmark-only {{ARGS}}


# Serve the python docs locally
py-docs-serve:
    mkdocs serve -f rerun_py/mkdocs.yml -w rerun_py

### Rust

# Generate and open the documentation for Rerun and all of its Rust dependencies.
#
# `--keep-going` makes sure we don't to abort the build process in case of errors.
# This is an unstable flag, available only on nightly.
rs-doc:
    cargo +nightly doc --all --open --keep-going --all-features -Zunstable-options

# `just rerun` is short a convenient shorthand, skipping the web viewer.
rerun *ARGS:
    cargo run --package rerun-cli --no-default-features --features native_viewer -- {{ARGS}}

# like `just rerun`, but with --release
rerun-release *ARGS:
    cargo run --package rerun-cli --no-default-features --features native_viewer --release -- {{ARGS}}

# `just rerun-web` is short a convenient shorthand for building & starting the web viewer.
rerun-web *ARGS:
    cargo run --package rerun-cli --no-default-features --features web_viewer -- --web-viewer {{ARGS}}

# like `rerun-web-release`, but with --release
rerun-web-release *ARGS:
    cargo run --package rerun-cli --no-default-features --features web_viewer --release -- --web-viewer {{ARGS}}

# Run the codegen. Optionally pass `--profile` argument if you want.
codegen *ARGS:
    pixi run codegen {{ARGS}}

# Print the contents of an .rrd file
print *ARGS:
    just rerun print {{ARGS}}

# To easily run examples on the web, see https://github.com/rukai/cargo-run-wasm.
# Temporary solution while we wait for our own xtasks!
run-wasm *ARGS:
    cargo run --release --package run_wasm -- {{ARGS}}

# Lint all of Rust code
rs-lint:
    #!/usr/bin/env bash
    set -euxo pipefail
    cargo cranky --quiet --all-features -- --deny warnings
    typos
    scripts/lint.py
    cargo doc --quiet --no-deps --all-features
    cargo doc --quiet --document-private-items --no-deps --all-features
    cargo test --quiet --doc --all-features # runs all doc-tests

# Lint Rust code for the wasm target
rs-lint-wasm:
    scripts/clippy_wasm.sh

# Run all examples with the specified args
rs-run-all *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    find examples/rust/ -name main.rs | xargs -I _ sh -c 'cd $(dirname _) && echo $(pwd) && cargo r'


### TOML

# Format .toml files
toml-format:
    taplo fmt

# Lint .toml files
toml-lint:
    taplo fmt --check --diff


### Misc

# Update the design_tokens.json used to style the GUI.
# See https://rerun-design-guidelines.netlify.app/tokens for their meanings.
# To update the upstream `design_tokens.json`, modify
# https://github.com/rerun-io/documentation/blob/main/src/utils/tokens.ts and push to main.
download-design-tokens:
    curl https://rerun-docs.netlify.app/api/tokens | jq > crates/re_ui/data/design_tokens.json

# Update the results of `insta` snapshot regression tests
update-insta-tests:
    cargo test; cargo insta review

upload *ARGS:
    python3 "scripts/upload_image.py" {{ARGS}}

crates *ARGS:
    python3 "scripts/ci/crates.py" {{ARGS}}
