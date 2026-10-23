<!--[metadata]
title = "Clock"
description = "An analog clock built from `Boxes3D`, `Points3D`, and `Arrows3D` primitives, animated along a Rerun timeline."
thumbnail = "https://static.rerun.io/clock_thumbnail/3882a4544fa0462410f7cf1e9598c527d7fbc4f2/768w.jpg"
thumbnail_dimensions = [768, 576]
-->


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
cmake --build . --target example_clock
./examples/cpp/clock/example_clock
```
