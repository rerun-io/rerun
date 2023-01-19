The main Rerun binary.

This can act either as a server, a viewer, or both, depending on which options you use when you start it.

`cargo run -p rerun -- --help`

## Hosting an SDK server
This will host an SDK server that SDK:s can connect to:

```sh
cargo run -p rerun
```

## Running a web viewer
The web viewer is an experimental feature, but you can try it out with:

```sh
cargo run -p rerun --features web -- --web-viewer ../nyud.rrd
```
