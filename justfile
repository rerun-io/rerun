# Install just: https://github.com/casey/just
#
# Then run `just --list` to see the available commands

export RUSTDOCFLAGS := "--deny warnings --deny rustdoc::missing_crate_level_docs"

default:
  @just --list


### Common
# Format all of our code
format: cpp-format toml-format py-format
    cargo fmt --all

# Lint all of our code
lint: toml-lint py-lint rs-lint

### C and C++

cpp-format:
    #!/usr/bin/env bash
    fd --extension h --exec clang-format -i
    fd --extension hpp --exec clang-format -i
    fd --extension c --exec clang-format -i
    fd --extension cpp --exec clang-format -i

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
    set -euxo pipefail
    # Note: proto.py relies on old-style annotation to work, and pyupgrade is too opinionated to be disabled from comments
    # See https://github.com/rerun-io/rerun/pull/2559 for details
    pyupgrade --py38-plus `find {{py_folders}} -name "*.py" -type f ! -path "examples/python/objectron/proto/objectron/proto.py"`
    # The order below is important and sadly we need to call black twice. Ruff does not yet
    # fix line-length (See: https://github.com/astral-sh/ruff/issues/1904).
    #
    # 1) Call black, which among others things fixes line-length
    # 2) Call ruff, which requires line-lengths to be correct
    # 3) Call black again to cleanup some whitespace issues ruff might introduce
    black --config rerun_py/pyproject.toml {{py_folders}}
    ruff --fix --config rerun_py/pyproject.toml  {{py_folders}}
    black --config rerun_py/pyproject.toml {{py_folders}}
    blackdoc {{py_folders}}

# Check that all the requirements.txt files for all the examples are correct
py-requirements:
    #!/usr/bin/env bash
    set -euo pipefail
    find examples/python/ -name main.py | xargs -I _ sh -c 'cd $(dirname _) && echo $(pwd) && pip-missing-reqs . || exit 255'

# Run linting
py-lint:
    #!/usr/bin/env bash
    set -euxo pipefail
    ruff check --config rerun_py/pyproject.toml  {{py_folders}}
    black --check --config rerun_py/pyproject.toml --diff {{py_folders}}
    blackdoc --check {{py_folders}}
    mypy --no-warn-unused-ignore

# Run fast unittests
py-test:
    python -m pytest -vv rerun_py/tests/unit/

# Run tests on all supported Python versions (through nox)
py-test-allpy:
    nox -s tests

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
    taplo fmt --check


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
