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

# Build and install the package into the venv
py-build mode="release":
    #!/usr/bin/env bash
    unset CONDA_PREFIX && \
        source venv/bin/activate && \
        maturin develop -m rerun_py/Cargo.toml --extras="tests" {{ if mode == "debug" { "" } else { "--release" } }}

# Run autoformatting
py-format:
    isort .
    black .
    blackdoc .
    pyupgrade --py37-plus `find rerun_sdk/ tests/ -name "*.py" -type f`
    cargo fmt --all

# Run linting
py-lint:
    #!/usr/bin/env bash
    cd rerun_py
    mypy
    flake8

# Run fast unittests
py-test:
    python -m pytest rerun_py/tests/unit/


### Rust

rs-doc:
    cargo +nightly doc --all --open --keep-going --all-features -Zunstable-options
