from __future__ import annotations

try:
    from rerun_bindings import (
        InvertedIndexQueryProperties as InvertedIndexQueryProperties,
        StorageNodeClient as StorageNodeClient,
        VectorDistanceMetric as VectorDistanceMetric,
        VectorIndexProperties as VectorIndexProperties,
        VectorIndexQueryProperties as VectorIndexQueryProperties,
        connect as connect,
    )

except ImportError as e:
    print("import failed: ", e)

    def connect(addr: str) -> StorageNodeClient:
        raise NotImplementedError("Rerun SDK was built without the `remote` feature enabled.")
