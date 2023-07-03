from __future__ import annotations

from dataclasses import dataclass

import pyarrow as pa


@dataclass
class Archetype:
    pass


class Component(pa.ExtensionArray):  # type: ignore[misc]
    @property
    def extension_name(self) -> str:
        return getattr(self, "_extension_name", "")
