from __future__ import annotations

try:
    from rerun_bindings import (
        connect as connect,
    )
except ImportError:

    def connect(url: str) -> None:
        raise NotImplementedError("Rerun SDK was built without the `remote` feature enabled.")
