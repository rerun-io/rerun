---
title: Spawn Viewer
tags: [spawn]
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/spawn_viewer/src/main.rs
---

Shows how to spawn a new Rerun Viewer process ready to listen for TCP connections using an executable available in PATH.

```bash
cmake .
cmake --build . --target spawn_viewer
./examples/cpp/spawn_viewer/spawn_viewer
```
