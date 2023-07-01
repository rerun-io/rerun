from __future__ import annotations

import pyarrow as pa


class Component(pa.ExtensionArray):  # type: ignore[misc]
    @property
    def extension_name(self) -> str:
        return getattr(self, "_extension_name", "")
