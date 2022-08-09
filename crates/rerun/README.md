The main Rerun binary.

This can act either as a server, a viewer, or both, depending on which options you use when you start it.

`cargo run --release -p rerun -- --help`

## Hosting an SDK server
This will host an SDK server that SDK:s can connect to:

```sh
RUST_LOG=debug cargo run -p rerun
```
