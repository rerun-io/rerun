The Rerun Python Log SDK.

Goal: an ergonomic Python library for logging rich data, over TCP, to a rerun server.

Note: The rust crate is called `re_sdk_python`, while the Python library is called `rerun_sdk`.

## Setup

Run this is in the workspace root to setup the python virtual environment:

```sh
source crates/re_sdk_python/setup_env.sh
```

The Python bindings is using https://github.com/PyO3/pyo3


## Testing
Debug build:
``` sh
maturin develop -m crates/re_sdk_python/Cargo.toml && RUST_LOG=debug python3 crates/re_sdk_python/test.py
```

Release build:
``` sh
maturin develop -m crates/re_sdk_python/Cargo.toml --release && RUST_LOG=debug python3 crates/re_sdk_python/test.py
```


### Using a remote viewer
First start up a viewer with a server that the logger can connect to:

```sh
RUST_LOG=debug cargo run -p rerun -- --host
```

Then run the test logging:

Debug build:
``` sh
maturin develop -m crates/re_sdk_python/Cargo.toml && RUST_LOG=debug python3 crates/re_sdk_python/test.py --connect
```


# TODO
* [ ] Add type annotations and use MyPy
