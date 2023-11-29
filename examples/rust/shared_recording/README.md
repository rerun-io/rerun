---
title: Shared Recording 
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/shared_recording/main.py
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/shared_recording/src/main.rs
cpp: https://github.com/rerun-io/rerun/tree/latest/examples/cpp/shared_recording/main.cpp
---

This example demonstrates how to use `RecordingId`s to build a single recording from multiple processes.

Run the following multiple times, and you'll see that each invokation adds data to the existing recording rather than creating a new one:
```bash
cargo run
```
