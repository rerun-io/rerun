<!--[metadata]
title = "Spawn Viewer"
tags = ["Spawn"]
-->


Shows how to spawn a new Rerun Viewer process ready to listen for gRPC connections using an executable available in PATH.

```bash
cmake .
cmake --build . --target spawn_viewer
./examples/cpp/spawn_viewer/spawn_viewer
```
