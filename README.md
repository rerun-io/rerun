# rerun
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

#### `comms`
WebSocket communication tools (encoding, decoding, client, server) between server and viewer.

#### `log_types`
The different types that make up the rerun log format.

#### `viewer`
`cargo run --release -p viewer -- --help`

Can run both on the web and natively. Talks to the server over WebSockets.
The viewer can also be used as a library.

#### `web_server`
A binary for serving the web viewer html and wasm.
