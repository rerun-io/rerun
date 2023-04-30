from __future__ import annotations

from typing import Sequence

import pyarrow as pa

from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "TextboxArray",
    "TextboxType",
]


class TextboxArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_bodies(text_entries: Sequence[tuple[str]]) -> TextboxArray:
        """Build a `TextboxArray` from a sequence of text bodies."""

        storage = pa.array(text_entries, type=TextboxType.storage_type)
        # TODO(jleibs) enable extension type wrapper
        # return cast(TextboxArray, pa.ExtensionArray.from_storage(TextboxType(), storage))
        return storage  # type: ignore[no-any-return]


TextboxType = ComponentTypeFactory("TextboxType", TextboxArray, REGISTERED_COMPONENT_NAMES["rerun.textbox"])

pa.register_extension_type(TextboxType())
