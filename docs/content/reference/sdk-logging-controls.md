---
title: SDK Logging Controls
order: 5
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

ℹ️ Note: When Rerun is disabled, logging statements are bypassed and essentially become no-ops.

## Creating a default-off setup in code

The "default-on" behavior can also be changed to a "default-off" behavior:

snippet: default-off-session

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
