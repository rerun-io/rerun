---
title: Standard Input/Output example
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/binary_file_loader/main.py
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/binary_file_loader/src/main.rs
cpp: https://github.com/rerun-io/rerun/tree/latest/examples/cpp/binary_file_loader/main.cpp
thumbnail: https://static.rerun.io/binary_file_loader/0e47ac513ab25d56cf2b493128097d499a07e5e8/480w.png
---

TODO
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/binary_file_loader/0e47ac513ab25d56cf2b493128097d499a07e5e8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/binary_file_loader/0e47ac513ab25d56cf2b493128097d499a07e5e8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/binary_file_loader/0e47ac513ab25d56cf2b493128097d499a07e5e8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/binary_file_loader/0e47ac513ab25d56cf2b493128097d499a07e5e8/1200w.png">
  <img src="https://static.rerun.io/binary_file_loader/0e47ac513ab25d56cf2b493128097d499a07e5e8/full.png" alt="Standard Input/Output example screenshot">
</picture>

Demonstrates how to log data to standard output with the Rerun SDK, and then visualize it from standard input with the Rerun Viewer.

```bash
echo 'hello from stdin!' | cargo run | rerun
```

