<!--[metadata]
title = "Minimal example"
thumbnail = "https://static.rerun.io/minimal-example/9e694c0689f20323ed0053506a7a099f7391afca/480w.png"
thumbnail_dimensions = [480, 480]
-->


<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/minimal/0e47ac513ab25d56cf2b493128097d499a07e5e8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/minimal/0e47ac513ab25d56cf2b493128097d499a07e5e8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/minimal/0e47ac513ab25d56cf2b493128097d499a07e5e8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/minimal/0e47ac513ab25d56cf2b493128097d499a07e5e8/1200w.png">
  <img src="https://static.rerun.io/minimal/0e47ac513ab25d56cf2b493128097d499a07e5e8/full.png" alt="Minimal example screenshot">
</picture>

The simplest example of how to use Rerun, showing how to log a point cloud.
This is part of the [Quick Start guide](https://www.rerun.io/docs/getting-started/data-in/rust).

```bash
cargo run --release
```
