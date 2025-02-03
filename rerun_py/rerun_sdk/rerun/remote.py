from __future__ import annotations

try:
    from rerun_bindings import (
        StorageNodeClient as StorageNodeClient,
        VectorDistanceMetric as VectorDistanceMetric,
        connect as connect,
    )

except ImportError as e:
    print("import failed: ", e)

    def connect(addr: str) -> StorageNodeClient:
        raise NotImplementedError("Rerun SDK was built without the `remote` feature enabled.")
