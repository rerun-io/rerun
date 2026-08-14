---
title: "Setting a recording id no longer disables recording properties"
hidden: true
type: breaking
---

### Setting a recording id no longer disables recording properties

Properties are now sent regardless of setting the recording id for Rust and C++, making it consistent with Python.

#### Rust

```rust
// Now sends properties:
let rec = rerun::RecordingStreamBuilder::new("rerun_example_my_app")
    .recording_id("run-1")
    .recording_started(epoch)
    .save("run-1.rrd")?;
```

To get the old behavior back, opt out explicitly:

```rust
// To get the old behavior:
let rec = rerun::RecordingStreamBuilder::new("rerun_example_my_app")
    .recording_id("run-1")
    .send_properties(false) // new
    .save("run-1.rrd")?;
```

C++ also has a new `send_properties` opt-out as a constructor parameter:

```cpp
// Sends properties:
const auto rec = rerun::RecordingStream("rerun_example_my_app", "run-1");

// Does not, same as old behavior:
const auto rec = rerun::RecordingStream("rerun_example_my_app", "run-1", rerun::StoreKind::Recording, false);
```

Docs: ../concepts/query-and-transform/properties-and-segments.md
