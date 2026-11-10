---
title: Logging Controls
order: 600
description: Turn logging to Rerun on or off, and control what Rerun prints to the console
---

## Controlling logging globally

Rerun logging is enabled by default. The logging behavior can be overridden at runtime using the `RERUN` environment variable:

```sh
export RERUN=off
python my_rerun_enabled_script.py
# or
cargo run my_rerun_package

# No log messages will be transmitted.
```

The `RERUN` environment variable is read once during SDK initialization. The accepted values for `RERUN` are `1/on/true`, and `0/off/false`.

> [!NOTE]
> When Rerun is disabled, logging statements are bypassed and essentially become no-ops.

## Creating a default-off setup in code

The "default-on" behavior can also be changed to a "default-off" behavior:

snippet: tutorials/default-off-session

## Dynamically turn logging on/off

In order to dynamically turn off logging at runtime, you can swap out the active recording with a disabled recording.
When you want to turn logging back on, you simply continue to use the previous recording again.

### Rust

In Rust you always pass the recording explicitly, making this fully transparent.
In order to create a no-op recording call `RecordingStream::disabled()`.

```rust
let noop_rec = RecordingStream::disabled();
```

### Python

The Python API uses the global recording stream by default.
To swap it out with a no-op recording call `set_global_data_recording` with `None`.

```python
# Disabling logging
prev_rec = rr.set_global_data_recording(None)

# …

# Re-enabling logging
rr.set_global_data_recording(prev_rec)
```

## Silencing console output

The settings above control the data you log to Rerun.
Rerun also prints its own messages (warnings, errors, and info) to the console.
You control those with the `RUST_LOG` environment variable:

```sh
RUST_LOG=error python my_script.py   # only show errors
RUST_LOG=off python my_script.py     # show nothing
RUST_LOG=debug rerun                 # show a lot more
```

The defaults are:

* Python SDK: `warn`.
* C++ SDK, and the `rerun` CLI and Viewer: `info`.
* Rust SDK: the SDK prints through the [`tracing`](https://crates.io/crates/tracing) crate, and does not set up a subscriber itself.
  What is printed depends on how your application sets up logging.

A Viewer started with `rr.spawn()` inherits the environment, including `RUST_LOG`, from the process that started it.
