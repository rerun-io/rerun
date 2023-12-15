---
title = "Custom Store Subscriber"
tags = ["store-event", "store-diff", "store-subscriber"]
---

<picture>
  <img src="https://static.rerun.io/custom_store_view/f7258673486f91d944180bd4a83307bce09b741e/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/custom_store_view/f7258673486f91d944180bd4a83307bce09b741e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/custom_store_view/f7258673486f91d944180bd4a83307bce09b741e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/custom_store_view/f7258673486f91d944180bd4a83307bce09b741e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/custom_store_view/f7258673486f91d944180bd4a83307bce09b741e/1200w.png">
</picture>

This example demonstrates how to use [`StoreSubscriber`]s and [`StoreEvent`]s to implement both custom secondary indices and trigger systems.

Usage:
```sh
# Start the Rerun Viewer with our custom view in a terminal:
$ cargo r -p custom_store_subscriber

# Log any kind of data from another terminal:
$ cargo r -p objectron -- --connect
```