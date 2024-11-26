from __future__ import annotations

try:
    from rerun_bindings import (
        StorageNodeClient as StorageNodeClient,
        connect as connect,
    )
except ImportError:

    def connect(addr: str) -> StorageNodeClient:
        raise NotImplementedError("Rerun SDK was built without the `remote` feature enabled.")
