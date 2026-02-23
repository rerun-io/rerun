---
title: Migrating from 0.29 to 0.30
order: 980
---

## 🐍 Python API

### `segment_url_udf` and `segment_url_with_timeref_udf` have been removed

The `segment_url_udf()` and `segment_url_with_timeref_udf()` functions in
`rerun.utilities.datafusion.functions.url_generation` have been removed. Use `segment_url()` instead,
which covers both use cases:

Before:

```python
from rerun.utilities.datafusion.functions.url_generation import segment_url_udf, segment_url_with_timeref_udf

# Without timestamp
udf = segment_url_udf(dataset)
df.with_column("url", udf(col("rerun_segment_id")))

# With timestamp
udf = segment_url_with_timeref_udf(dataset, "my_timeline")
df.with_column("url", udf(col("rerun_segment_id"), col("ts"), lit("my_timeline")))
```

After:

```python
from rerun.utilities.datafusion.functions.url_generation import segment_url

# Without timestamp
df.with_column("url", segment_url(dataset))

# With timestamp
df.with_column("url", segment_url(dataset, timestamp="ts", timeline_name="my_timeline"))
```

Also, the previously deprecated `partition_url()`, `partition_url_udf()`, and `partition_url_with_timeref_udf()`
function have been removed.

### `segment_url` parameter names have been updated

The `_col` suffix has been removed from all parameters of `segment_url()` since they accept any DataFusion
expression, not just column references:

| Old name         | New name     |
|------------------|--------------|
| `segment_id_col` | `segment_id` |
| `timestamp_col`  | `timestamp`  |

The newly introduced arguments `time_range_start`, `time_range_end`, and `selection` follow the same pattern.


## CLI

### `.rrd` files are no longer tailed by default

Previously, when opening an `.rrd` file from the command line, the viewer would keep watching the file
for new data past EOF (tailing), which is useful when a writer process is still appending to the file.

Starting with 0.30, `.rrd` files are read once and loading stops at EOF.
To restore the old tailing behavior, pass the `--follow` flag:

```sh
rerun --follow recording.rrd
```

### `SeriesVisible` component type has been removed

The `SeriesVisible` component has been removed in favor of the existing `Visible` component.
If you were using `SeriesVisible` to control visibility of `SeriesLines` or `SeriesPoints`,
use `Visible` instead. Existing recordings are migrated automatically.
