# The Rerun Python Log SDK.

Goal: an ergonomic Python library for logging rich data, over TCP, to a rerun server.

Note: The rust crate is called `re_sdk_python`, while the Python library is called `rerun_sdk`.

## Setup

Note: these instructions all assume you're running them from the root of the rerun repository.

Make sure you have a virtualenv set up.

```sh
python3 -m venv env
source env/bin/activate
python3 -m pip install --upgrade pip
```

The Python bindings are built using https://github.com/PyO3/pyo3


## Build, test, and run
### Build and install
```sh
pip install "crates/re_sdk_python[tests,examples]"
```
Note: `[tests,examples]` here is used to specify that we also want to install the dependencies needed for running tests and examples.

### Test
```sh
mypy crates/re_sdk_python  # Static type checking
pytest crates/re_sdk_python  # Unit tests
```

### Running the example code
```sh
python3 crates/re_sdk_python/example.py
```

### Using the viewer in a different process
By default, the example runs Rerun in buffered mode, in the same process as the example code. This means all logged data is buffered until `rerun_sdk.show()` is called in the end, which shows the viewer and blocks until the viewer is closed.

Rerun can aslo be run in non-blocking mode with viewer and logger in different processes.

First, start up a viewer with a server that the SDK can connect to:
```sh
cargo run -p rerun
```

Then, run the example with the `--connect` option:
```sh
python3 crates/re_sdk_python/example.py --connect
```

## Developing
For ease of development you can build and install in "editable" mode by passing `-e` (or `--editable` ) to `pip install`. This means you can edit the `rerun_sdk` Python code without having to re-build and install to see changes.
```sh
pip install -e "crates/re_sdk_python[tests,examples]"
```

## Building an installable Python Wheel

To build an installable Python wheel
```
pip install -r crates/re_sdk_python/requirments-build.txt
maturin build -m ./crates/re_sdk_python/Cargo.toml --release
```

By default the wheels will be built to `target/wheels`.

Now you can install `rerun_sdk` in any Python3 environment using:

```
pip3 install target/wheels/*.whl
```


## Troubleshooting
You can run with `RUST_LOG=debug` to get more output out of the rerun SDK.
