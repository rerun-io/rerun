---
title: Rust SDK
order: 300
description: Set up the Rust SDK and reduce its dependencies and compile time
---

Add the [Rerun crate](https://crates.io/crates/rerun) using `cargo add rerun`.

You'll additionally need to install the [Viewer](./viewer.md).

## Reduce dependencies and compile time

The default features of the `rerun` crate include a lot more than the logging SDK, such as the gRPC server (`server`), the dataframe API (`dataframe`), file importers (`importers`), and support for video, images, audio, and the map view.
If you only need to log data, turn off the default features and enable only `sdk`:

```toml
[dependencies]
rerun = { version = "…", default-features = false, features = ["sdk"] }
```

Then add back the features you need, for example `image` for the `image` crate integration, or `glam` for math type conversions.
See the [list of features](https://docs.rs/crate/rerun/latest/features).

The native Viewer is not part of the default features.
Only enable `native_viewer` if you want to embed or extend the Viewer in your own app, since it adds many dependencies.
To show your data, `spawn()` starts the `rerun` binary that you [installed separately](./viewer.md).

Debug builds of Rerun run slowly.
To make them faster, compile your dependencies with optimizations, in your `Cargo.toml`:

```toml
[profile.dev.package."*"]
opt-level = 2
```

Your own code is still built without optimizations, and dependencies are only rebuilt when they change.

## Next steps

[Set up a Rust project](../../getting-started/project-setup/rust.md), then walk through the [Log and Ingest](../../logging-and-ingestion/tutorial.md) tutorial.

-   [Rust API docs](https://docs.rs/rerun/latest/rerun/)
