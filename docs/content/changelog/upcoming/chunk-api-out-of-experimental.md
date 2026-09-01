---
title: "Chunk API and stable readers moved out of `experimental`"
hidden: true
type: breaking
---

### Chunk API and stable readers moved out of `rerun.experimental`

The following API were promoted from the `rerun.experimental` to `rerun.chunk` namespace.

The old `rerun.experimental` names still work but emit a `DeprecationWarning` and forward to `rerun.chunk`. They will be removed one release from now.

Moved to `rerun.chunk`: `Chunk`, `ChunkStore`, `LazyChunkStream`, `LazyStore`, `StoreEntry`, `Lens`, `DeriveLens`, `MutateLens`, `Selector`, `IndexColumn`, `OptimizationProfile`, `RrdReader`, `McapReader` (and the `Mcap*Info` types), plus the reader protocols `StreamingReader` and `IndexedReader`.

`send_chunks` is promoted to the top level as `rr.send_chunks`.

```py
from rerun.experimental import Chunk, RrdReader  # before
from rerun.chunk import Chunk, RrdReader  # after
```

Mixed imports split by where each name now lives:

```py
from rerun.experimental import Chunk, McapReader, ParquetReader  # before
from rerun.chunk import Chunk, McapReader  # after
from rerun.experimental import ParquetReader  # after
```
