<!--[metadata]
title = "Log file example"
-->

Demonstrates how to log any file from the SDK using the [`Importer`](https://www.rerun.io/docs/concepts/logging-and-ingestion/importers/overview?speculative-link) machinery.

To build it from a checkout of the repository (requires a Rust toolchain):
```bash
cmake .
cmake --build . --target example_log_file
./examples/cpp/log_file/example_log_file examples/assets/
```
