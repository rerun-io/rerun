<--[metadata]
title = "Objectron"
tags = ["2D", "3D", "object-detection", "pinhole-camera"]
thumbnail = "https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/480w.png"
build_args = ["--frames=100"]
-->

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/1200w.png">
  <img src="https://static.rerun.io/objectron/8ea3a37e6b4af2e06f8e2ea5e70c1951af67fea8/full.png" alt="Objectron example screenshot">
</picture>

Example of using the Rerun SDK to log the [Objectron](https://github.com/google-research-datasets/Objectron) dataset.

> The Objectron dataset is a collection of short, object-centric video clips, which are accompanied by AR session metadata that includes camera poses, sparse point-clouds and characterization of the planar surfaces in the surrounding environment.

```bash
cargo run --release
```
