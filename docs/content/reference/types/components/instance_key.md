---
title: "InstanceKey"
---

A unique numeric identifier for each individual instance within a batch.

Instance keys are automatically assigned by the `rerun` library and should not be set manually.

The instance key is just the index of the instance within the batch,
i.e. the first point in a point cloud has `InstanceKey = 0`, the second `InstanceKey = 1`, and so on.

We plan to remove the `InstanceKey` component in the near future.

## Fields

* value: `u64`

## Links
 * 🌊 [C++ API docs for `InstanceKey`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1components_1_1InstanceKey.html)
 * 🐍 [Python API docs for `InstanceKey`](https://ref.rerun.io/docs/python/stable/common/components#rerun.components.InstanceKey)
 * 🦀 [Rust API docs for `InstanceKey`](https://docs.rs/rerun/latest/rerun/components/struct.InstanceKey.html)


