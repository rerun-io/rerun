---
title: Migrating from 0.29 to 0.30
order: 980
---

## 🐍 Python API

### Deprecated UDF have been removed

The deprecated `partition_url`, `partition_url_udf`, and `partition_url_with_timeref_udf` functions in
`rerun.utilities.datafusion.functions.url_generation` have been removed. Use the `segment_url` equivalents instead:

| Removed                            | Replacement                      |
|------------------------------------|----------------------------------|
| `partition_url()`                  | `segment_url()`                  |
| `partition_url_udf()`              | `segment_url_udf()`              |
| `partition_url_with_timeref_udf()` | `segment_url_with_timeref_udf()` |
