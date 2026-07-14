from __future__ import annotations

from datafusion import col

# region: build_store
import rerun as rr
from rerun.experimental import Chunk, ChunkStore

chunk = Chunk.from_columns(
    "/sensor",
    indexes=[rr.TimeColumn("frame", sequence=[0, 1, 2, 3])],
    columns=rr.Scalars.columns(scalars=[0.0, 0.5, 1.0, 1.5]),
)
store = ChunkStore.from_chunks([chunk])
# endregion: build_store

# region: query
df = store.reader(index="frame")
df = df.filter(col("/sensor:Scalars:scalars")[0] >= 1.0)
print(df)  # or convert to Pandas, Polars, PyArrow, etc.
# endregion: query
