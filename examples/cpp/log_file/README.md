<!--[metadata]
title = "Log file example"
description = "How to log any file from the SDK using the Importer machinery, a one-call path for assets the viewer understands."
-->

Demonstrates how to log any file from the SDK using the [`Importer`](https://www.rerun.io/docs/concepts/logging-and-ingestion/importers/overview) machinery.

To build it from a checkout of the repository (requires a Rust toolchain):
```bash
cmake .
cmake --build . --target example_log_file
./examples/cpp/log_file/example_log_file examples/assets/
```
