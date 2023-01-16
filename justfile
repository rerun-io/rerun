default:
  @just --list


### Common
# Format all of our code
format: toml-format py-format
    cargo fmt --all

# Lint all of our code
lint: toml-lint py-lint
    cargo cranky
    scripts/lint.py


### Python

# Set up a Pythonvirtual environment for development
py-dev-env:
    #!/usr/bin/env bash
    echo "Setting up Python virtual environment in venv"
    # set -euxo pipefail
    python3 -m venv venv
    venv/bin/pip install --upgrade pip
    venv/bin/pip install -r rerun_py/requirements-build.txt
    venv/bin/pip install -r rerun_py/requirements-lint.txt
    echo "Do 'source venv/bin/activate' to use the virtual environment!"

# Run all examples
py-run-all: py-build
    fd main.py | xargs -I _ sh -c "echo _ && python3 _"

# Build and install the package into the venv
py-build:
    #!/usr/bin/env bash
    unset CONDA_PREFIX && \
        source venv/bin/activate && \
        maturin develop \
            -m rerun_py/Cargo.toml \
            --extras="tests"

# Run autoformatting
py-format:
    black --config rerun_py/pyproject.toml .
    blackdoc .
    isort .
    pyupgrade --py37-plus `find rerun_py/rerun/ -name "*.py" -type f`

# Run linting
py-lint:
    black --check --config rerun_py/pyproject.toml --diff .
    blackdoc --check .
    isort --check .
    mypy --no-warn-unused-ignore
    flake8

# Run fast unittests
py-test:
    python -m pytest rerun_py/tests/unit/

### Rust

# Generate and open the documentation for Rerun and all of its Rust dependencies.
#
# `--keep-going` makes sure we don't to abort the build process in case of errors.
# This is an unstable flag, available only on nightly.
rs-doc:
    cargo +nightly doc --all --open --keep-going --all-features -Zunstable-options


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
download-design-tokens:
    curl https://rerun-design-guidelines.netlify.app/api/tokens | jq > crates/re_ui/data/design_tokens.json
