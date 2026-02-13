---
title: Send entire columns at once
order: 100
description: How to use the Rerun SDK to log big chunks of data in one call
---

The [`log` API](../../getting-started/data-in/python.md#logging-our-first-points) is designed to extract data from your running code as it's being generated. It is, by nature, *row-oriented*.
If you already have data stored in something more *column-oriented*, it can be both a lot easier and more efficient to send it to Rerun in that form directly.

This is what the `send_columns` API is for: it lets you efficiently update the state of an entity over time, sending data for multiple index and component columns in a single operation.

> ⚠️ `send_columns` API bypasses the time context and [micro-batcher](../../reference/sdk/micro-batching.md) ⚠️
>
> In contrast to the `log` API, `send_columns` does NOT add any other timelines to the data. Neither the built-in timelines `log_time` and `log_tick`, nor any [user timelines](../../concepts/logging-and-ingestion/timelines.md). Only the timelines explicitly included in the call to `send_columns` will be included.

To learn more about the concepts behind the columnar APIs, and the Rerun data model in general, [refer to this page](../../concepts/logging-and-ingestion/chunks.md).


## Reference

* [🌊 C++](https://ref.rerun.io/docs/cpp/stable/classrerun_1_1RecordingStream.html#ad17571d51185ce2fc2fc2f5c3070ad65)
* [🐍 Python](https://ref.rerun.io/docs/python/stable/common/columnar_api/#rerun.send_columns)
* [🦀 Rust](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.send_columns)


## Examples


### Updating a scalar over time, in a single operation

Consider this snippet, using the row-oriented `log` API:

snippet: archetypes/scalars_row_updates

which can be translated to the column-oriented `send_columns` API as such:

snippet: archetypes/scalars_column_updates

<picture data-inline-viewer="snippets/archetypes/scalars_column_updates">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/1200w.png">
  <img src="https://static.rerun.io/transform3d_column_updates/2b7ccfd29349b2b107fcf7eb8a1291a92cf1cafc/full.png">
</picture>


### Updating a point cloud over time, in a single operation

Consider this snippet, using the row-oriented `log` API:

snippet: archetypes/points3d_row_updates

which can be translated to the column-oriented `send_columns` API as such:

snippet: archetypes/points3d_column_updates

<picture data-inline-viewer="snippets/archetypes/points3d_column_updates">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/1200w.png">
  <img src="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/full.png">
</picture>


Each row in the component column can be a batch of data, e.g. a batch of positions.
This lets you log the evolution of a point cloud over time efficiently.

### Updating a fixed number of arrows over time, in a single operation

Consider this snippet, using the row-oriented `log` API:

snippet: archetypes/arrows3d_row_updates

which can be translated to the column-oriented `send_columns` API as such:

snippet: archetypes/arrows3d_column_updates

<picture data-inline-viewer="snippets/archetypes/arrows3d_column_updates">
  <img src="https://static.rerun.io/arrows3d_column_updates/3e14b35aac709e3f1352426bd905c635b1e13879/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/arrows3d_column_updates/3e14b35aac709e3f1352426bd905c635b1e13879/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/arrows3d_column_updates/3e14b35aac709e3f1352426bd905c635b1e13879/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/arrows3d_column_updates/3e14b35aac709e3f1352426bd905c635b1e13879/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/arrows3d_column_updates/3e14b35aac709e3f1352426bd905c635b1e13879/1200w.png">
</picture>

Each row in the component column can be a batch of data, e.g. a batch of positions.
This lets you log the evolution of a set of arrows over time efficiently.


### Updating a transform over time, in a single operation

Consider this snippet, using the row-oriented `log` API:

snippet: archetypes/transform3d_row_updates

which can be translated to the column-oriented `send_columns` API as such:

snippet: archetypes/transform3d_column_updates

<picture data-inline-viewer="snippets/archetypes/transform3d_column_updates">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/1200w.png">
  <img src="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/full.png">
</picture>


### Updating an image over time, in a single operation

Consider this snippet, using the row-oriented `log` API:

snippet: archetypes/image_row_updates

which can be translated to the column-oriented `send_columns` API as such:

snippet: archetypes/image_column_updates

<picture data-inline-viewer="snippets/archetypes/image_column_updates">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/image_column_updates/8edcdc512f7b97402f03c24d7dcbe01b3651f86d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/image_column_updates/8edcdc512f7b97402f03c24d7dcbe01b3651f86d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/image_column_updates/8edcdc512f7b97402f03c24d7dcbe01b3651f86d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/image_column_updates/8edcdc512f7b97402f03c24d7dcbe01b3651f86d/1200w.png">
  <img src="https://static.rerun.io/image_column_updates/8edcdc512f7b97402f03c24d7dcbe01b3651f86d/full.png">
</picture>


### Updating custom user-defined values over time, in a single operation

[User-defined data](./custom-data.md) can also benefit from the column-oriented APIs.

Consider this snippet, using the row-oriented `log` API:

snippet: howto/any_values_row_updates

which can be translated to the column-oriented `send_columns` API as such:

snippet: howto/any_values_column_updates
