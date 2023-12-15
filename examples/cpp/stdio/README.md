---
title = "Standard Input/Output example"
thumbnail = "https://static.rerun.io/stdio/25c5aba992d4c8b3861386d8d9539a4823dca117/480w.png"
---

<picture>
  <img src="https://static.rerun.io/stdio/25c5aba992d4c8b3861386d8d9539a4823dca117/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/stdio/25c5aba992d4c8b3861386d8d9539a4823dca117/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/stdio/25c5aba992d4c8b3861386d8d9539a4823dca117/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/stdio/25c5aba992d4c8b3861386d8d9539a4823dca117/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/stdio/25c5aba992d4c8b3861386d8d9539a4823dca117/1200w.png">
</picture>

Demonstrates how to log data to standard output with the Rerun SDK, and then visualize it from standard input with the Rerun Viewer.

To build it from a checkout of the repository (requires a Rust toolchain):
```bash
cmake .
cmake --build . --target example_stdio
echo 'hello from stdin!' | ./examples/cpp/stdio/example_stdio | rerun -
```