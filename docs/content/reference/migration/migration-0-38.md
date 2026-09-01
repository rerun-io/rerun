---
title: Migrating from 0.37 to 0.38
order: 976
hidden: true
---

## Chunk API and stable readers moved out of `experimental`

The chunk API and some readers were promoted from `rerun.experimental` to a new flat `rerun.chunk` namespace.

The old `rerun.experimental` names still work but emit a `DeprecationWarning` and forward to `rerun.chunk`.
They will be removed one release from now — a single-release deprecation, since `experimental` never promised stability.

### Moved to `rerun.chunk`

`Chunk`, `ChunkStore`, `LazyChunkStream`, `LazyStore`, `StoreEntry`, `Lens`, `DeriveLens`, `MutateLens`, `Selector`, `IndexColumn`, `OptimizationProfile`, `RrdReader`, `McapReader` (and the `Mcap*Info` types), plus the reader protocols `StreamingReader` and `IndexedReader`.

```py
from rerun.experimental import Chunk, RrdReader  # before
from rerun.chunk import Chunk, RrdReader  # after
```

### `send_chunks` promoted to the top level

`send_chunks` moved to the top level as `rr.send_chunks`, next to `rr.send_dataframe`.

```py
from rerun.experimental import send_chunks  # before
from rerun import send_chunks  # after
```

### Names that stay experimental

The bleeding-edge readers (`Hdf5Reader`, `ParquetReader`, `Mp4Reader`) and `ViewerClient` / `query_metrics` stay in `rerun.experimental`.
Split a mixed import by where each name now lives.

```py
from rerun.experimental import Chunk, McapReader, ParquetReader  # before
from rerun.chunk import Chunk, McapReader  # after
from rerun.experimental import ParquetReader  # after
```
