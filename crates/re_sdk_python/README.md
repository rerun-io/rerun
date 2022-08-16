The Rerun Python Log SDK.

Goal: an ergonomic Python library for logging rich data, over TCP, to a rerun server.

Note: The rust crate is called `re_sdk_python`, while the Python library is called `rerun_sdk`.

## Setup

Run this is in the workspace root to setup the python virtual environment:

```sh
source crates/re_sdk_python/setup_env.sh
```

The Python bindings is using https://github.com/PyO3/pyo3


## Building and running
Build:
```sh
maturin develop -m crates/re_sdk_python/Cargo.toml --release
```

Running the example code:
```sh
python3 crates/re_sdk_python/example.py
```


### Using a remote viewer
First start up a viewer with a server that the SDK can connect to:

```sh
cargo run -p rerun
```

Then build and run the test logging:

Debug build:
```sh
maturin develop -m crates/re_sdk_python/Cargo.toml --release
python3 crates/re_sdk_python/example.py --connect
```


## Installing the Rerun Python SDK

First build it:
```
source crates/re_sdk_python/setup_env.sh
maturin build -m ./crates/re_sdk_python/Cargo.toml --release
```

Now you can install `rerun_sdk` in any Python3 environment using:

```
pip3 install PATH_TO_RERUN_REPOSITORY/target/wheels/*.whl
```


## Troubleshooting
You can run with `RUST_LOG=debug` to get more output out of the rerun SDK.
