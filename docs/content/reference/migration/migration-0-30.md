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
df.with_column("url", segment_url(dataset, timestamp_col="ts", timeline_name="my_timeline"))
```

Also, the previously deprecated `partition_url()`, `partition_url_udf()`, and `partition_url_with_timeref_udf()`
function have been removed.
