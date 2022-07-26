The Rerun Python Log SDK.

Goal: an ergonomic Python library for logging rich data, over TCP, to a rerun server.

Note: The rust crate is called `re_sdk_python`, while the Python library is called `rerun_sdk`.

## Setup

Run this is in the workspace root:

```
python3 -m venv env
source env/bin/activate
python3 -m pip install -r crates/re_sdk_python/requirements.txt
```

The Python bindings is using https://github.com/PyO3/pyo3


## Testing
First start up a viewer with a server that the logger can connect to:

```sh
(cd crates/re_viewer && RUST_LOG=debug cargo r --features server -- --host)
```

Then run the test logging:

Debug build:
``` sh
maturin develop -m crates/re_sdk_python/Cargo.toml && RUST_LOG=debug python3 test.py
```

Release build:
``` sh
maturin develop -m crates/re_sdk_python/Cargo.toml --release && RUST_LOG=debug python3 test.py
```
