The main Rerun logging API and binary.

## Library
You can add the `rerun` crate to your project with `cargo add rerun` (soon)
<!-- TODO(#1161): remove the (soon) -->

To get started, see [the examples](https://github.com/rerun-io/rerun/tree/main/examples).
<!-- TODO(#1161): update link to point to the rust examples -->

## Binary
You can install the binary with `cargo install rerun` (soon)
<!-- TODO(#1161): remove the (soon) -->

This can act either as a server, a viewer, or both, depending on which options you use when you start it.

`cargo run -p rerun -- --help`

### Hosting an SDK server
This will host an SDK server that SDK:s can connect to:

```sh
cargo run -p rerun
```

### Running a web viewer
The web viewer is an experimental feature, but you can try it out with:

```sh
cargo run -p rerun --features web -- --web-viewer ../nyud.rrd
```
