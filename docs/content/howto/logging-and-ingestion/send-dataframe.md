---
title: Send dataframes to a recording
order: 120
description: Log a whole table into a recording in a single call
---

If your data is already in a table, you can log the whole table in a single call instead of logging it row by row or column by column.
In Python, `rr.send_dataframe` takes a pandas `DataFrame` or an Arrow table and logs it into the active recording, so it's saved with the rest of your data and can be shown next to your other views.

Pass the timeline columns as `index` and the desired entity path for the remaining columns as `entity_path`:

snippet: howto/send_dataframe

Each remaining column becomes a component of that entity, which you can display in a [`DataframeView`](../../reference/types/views/dataframe_view.md).

Like [`send_columns`](send-columns.md), this ignores any timeline values set with `rr.set_time` and bypasses the [micro-batcher](../../reference/sdk/micro-batching.md): only the timelines in the table are added.
To control component types or spread columns across several entities, see [Chunks](../../concepts/logging-and-ingestion/chunks.md).

## Reference

* [🐍 Python `send_dataframe`](https://ref.rerun.io/docs/python/stable/common/columnar_api/#rerun.send_dataframe)
