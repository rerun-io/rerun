The Rerun viewer.

This is the main crate with all the GUI.

This is both a library and a binary. Can be compiled both natively for desktop, and as WASM for web.

Talks to the server over WebSockets (using `re_ws_comms`).

`cargo run --release -p re_viewer -- --help`

## Hosting an SDK server
This will host an SDK server inline in the viewer, that SDK:s can connect to:

```sh
(cd crates/re_viewer && RUST_LOG=debug cargo r --features server -- --host)
```
