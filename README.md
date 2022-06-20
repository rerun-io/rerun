# rerun
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/rerun/blob/master/LICENSE-APACHE)

This repository contains the rerun log SDK and viewer.

## Setup
Install Rust: https://rustup.rs/

``` sh
./setup.sh
```

## Check
``` sh
./check.sh
```

### Other
You can view higher log levels with `export RUST_LOG=debug` or  `export RUST_LOG=trace`.


## Crates

#### `re_comms`
WebSocket communication tools (encoding, decoding, client, server) between server and viewer.

#### `re_data_store`
In-memory storage of log data, indexed for fast fast queries.

#### `re_log_types`
The different types that make up the rerun log format.

#### `re_viewer`
`cargo run --release -p re_viewer -- --help`

Can run both on the web and natively. Talks to the server over WebSockets.
The viewer can also be used as a library.

#### `re_web_server`
A binary for serving the web viewer html and wasm.
