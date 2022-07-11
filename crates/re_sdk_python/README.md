The Rerun Python Log SDK.

Goal: an ergonomic Python library for logging rich data, over TCP, to a rerun server.

Note: The rust crate is called `re_sdk_python`, while the Python library is called `rerun_sdk`.

## Setup

Run this is in the workspace root:

```
python3 -m venv env
source env/bin/activate
python3 -m pip install maturin
```

The Python bindings is using https://github.com/PyO3/pyo3


## Testing
Debug build:
``` sh
(cd crates/re_sdk_python && maturin develop) && RUST_LOG=debug python3 test.py
```

Release build:
``` sh
(cd crates/re_sdk_python && maturin develop) --release && RUST_LOG=debug python3 test.py
```
