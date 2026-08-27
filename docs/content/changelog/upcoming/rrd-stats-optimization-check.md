---
title: "`rerun rrd stats` tells you whether a recording should be optimized"
hidden: true
type: feature
---

### `rerun rrd stats` tells you whether a recording should be optimized

`rerun rrd stats` now ends with a `Chunk index analysis` section, computed per store from the chunk index alone:

```
Chunk index analysis
--------------------

Store StoreId(Recording, "droid", "WEIRD_5047dd9a_2024_01_21_23h_22m_28s")
chunk index columns: 66

Optimization check based on a 2.0 MiB chunk size target (`--profile object-store`)
  - theoretical lower bound:   1 149 chunks
  - effective:               584 409 chunks (508.6×)
  - excess:                  583 260 chunks

⚠️ This recording may be unoptimized — consider running `rerun rrd optimize`
```

The check compares the recording's temporal chunk count against a lower bound of what merging could achieve under the object-store optimization profile.
When both the relative and absolute excess cross a threshold, the output warns about possibly unoptimized data.

The analysis requires a chunk index; run `rerun rrd migrate` first on files written before those existed.

Docs: ../howto/logging-and-ingestion/optimize-chunks
