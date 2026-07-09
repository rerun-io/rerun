"""Pluggable local vector-store backends for the DROID frame index.

`ingest.py` and `search.py` talk to a vector store only through the small
`VectorStore` interface here, so the same code path works whether you pick
LanceDB (`--backend lance`) or Qdrant (`--backend qdrant`). Both run fully
locally on disk — no server to stand up — so the example stays one-command.
Adding another store (Pinecone, pgvector, Milvus, …) is a third subclass.

The index is written from the columnar `(segment_id, camera, timestamp_ms,
vector)` Arrow table that `ingest.py` assembles. Searches return hits carrying
those three metadata fields plus a `similarity` in `[-1, 1]` (cosine),
normalized here so callers never have to know which backend's distance/score
convention is in play.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import pyarrow as pa

BACKENDS = ("lance", "qdrant")

# Per-backend default on-disk location, used when `--db-path` is omitted.
DEFAULT_PATHS = {
    "lance": "./droid_lancedb",
    "qdrant": "./droid_qdrant",
}


class VectorStore(ABC):
    """A local on-disk vector index over `(segment_id, camera, timestamp_ms, vector)` rows."""

    @abstractmethod
    def write(self, table: pa.Table) -> None:
        """(Over)write the index from *table*, replacing any existing table/collection.

        *table* has columns `segment_id` (string), `camera` (string),
        `timestamp_ms` (int64), and `vector` (fixed-size list of float32).
        """

    @abstractmethod
    def search(self, vector: list[float], top_k: int) -> list[dict[str, Any]]:
        """Cosine nearest-neighbor search.

        Returns up to *top_k* hit dicts, each with `segment_id`, `camera`,
        `timestamp_ms`, and a cosine `similarity` (higher is closer).
        """


def open_store(backend: str, path: str, table: str) -> VectorStore:
    if backend == "lance":
        return LanceStore(path, table)
    if backend == "qdrant":
        return QdrantStore(path, table)
    raise ValueError(f"Unknown backend '{backend}'; expected one of {BACKENDS}.")


class LanceStore(VectorStore):
    """LanceDB backend. Returns cosine *distance*, which we flip to similarity."""

    def __init__(self, path: str, table: str) -> None:
        self._path = path
        self._table = table

    def write(self, table: pa.Table) -> None:
        import lancedb

        dim = table.schema.field("vector").type.list_size

        db = lancedb.connect(self._path)
        tbl = db.create_table(self._table, data=table, mode="overwrite")
        print(f"Wrote {table.num_rows} rows ({dim}-dim) to LanceDB table '{self._table}' in {self._path}")

        # An ANN index needs enough rows to train; small demo tables fall back to
        # brute-force search, which is exact and plenty fast at this scale.
        try:
            tbl.create_index(metric="cosine", vector_column_name="vector")
            print("Built ANN index (cosine).")
        except Exception as exc:
            print(f"Skipped ANN index ({exc}); brute-force cosine search will be used.")

    def search(self, vector: list[float], top_k: int) -> list[dict[str, Any]]:
        import lancedb

        tbl = lancedb.connect(self._path).open_table(self._table)
        hits = tbl.search(vector).metric("cosine").limit(top_k).to_list()
        return [
            {
                "segment_id": h["segment_id"],
                "camera": h["camera"],
                "timestamp_ms": h["timestamp_ms"],
                "similarity": 1.0 - float(h["_distance"]),  # cosine distance -> similarity
            }
            for h in hits
        ]


class QdrantStore(VectorStore):
    """Qdrant backend in local (embedded) mode. Returns cosine *score* directly."""

    def __init__(self, path: str, collection: str) -> None:
        self._path = path
        self._collection = collection

    def write(self, table: pa.Table) -> None:
        from qdrant_client import QdrantClient, models

        dim = table.schema.field("vector").type.list_size
        segment_ids = table.column("segment_id").to_pylist()
        cameras = table.column("camera").to_pylist()
        timestamps_ms = table.column("timestamp_ms").to_pylist()
        vectors = table.column("vector").to_pylist()

        client = QdrantClient(path=self._path)

        # Mirror Lance's overwrite semantics: drop any prior collection first.
        if client.collection_exists(self._collection):
            client.delete_collection(self._collection)
        client.create_collection(
            collection_name=self._collection,
            vectors_config=models.VectorParams(size=dim, distance=models.Distance.COSINE),
        )

        points = [
            models.PointStruct(
                id=i,
                vector=vectors[i],
                payload={
                    "segment_id": segment_ids[i],
                    "camera": cameras[i],
                    "timestamp_ms": timestamps_ms[i],
                },
            )
            for i in range(table.num_rows)
        ]
        client.upsert(collection_name=self._collection, points=points)
        print(f"Wrote {table.num_rows} rows ({dim}-dim) to Qdrant collection '{self._collection}' in {self._path}")

    def search(self, vector: list[float], top_k: int) -> list[dict[str, Any]]:
        from qdrant_client import QdrantClient

        client = QdrantClient(path=self._path)
        result = client.query_points(
            collection_name=self._collection,
            query=vector,
            limit=top_k,
            with_payload=True,
        )
        return [
            {
                "segment_id": payload["segment_id"],
                "camera": payload["camera"],
                "timestamp_ms": payload["timestamp_ms"],
                "similarity": float(point.score),  # Qdrant cosine score is already a similarity
            }
            for point in result.points
            if (payload := point.payload) is not None
        ]
