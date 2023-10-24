---
title: Clock
python: https://github.com/rerun-io/rerun/tree/latest/examples/python/clock/main.py
rust: https://github.com/rerun-io/rerun/tree/latest/examples/rust/clock/src/main.rs
# TODO(#3962): Update Link
cpp: https://github.com/rerun-io/rerun/tree/main/examples/cpp/clock/main.cpp
thumbnail: https://static.rerun.io/clock/ae4b8970edba8480431cb71e57b8cddd9e1769c7/480w.png
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/clock/05e69dc20c9a28005f1ffe7f0f2ac9eeaa95ba3b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/clock/05e69dc20c9a28005f1ffe7f0f2ac9eeaa95ba3b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/clock/05e69dc20c9a28005f1ffe7f0f2ac9eeaa95ba3b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/clock/05e69dc20c9a28005f1ffe7f0f2ac9eeaa95ba3b/1200w.png">
  <img src="https://static.rerun.io/clock/05e69dc20c9a28005f1ffe7f0f2ac9eeaa95ba3b/full.png" alt="Clock example screenshot">
</picture>

An example visualizing an analog clock with hour, minute and seconds hands using Rerun Arrow3D primitives.


To build it from a checkout of the repository (requires a Rust toolchain):
```bash
cmake .
cmake --build . --target clock_example
./examples/cpp/clock/clock_example
```
