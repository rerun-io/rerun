from __future__ import annotations

from typing import Sequence

import pyarrow as pa
from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory

__all__ = [
    "TextEntryArray",
    "TextEntryType",
]


class TextEntryArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_bodies_and_levels(text_entries: Sequence[tuple[str, str | None]]) -> TextEntryArray:
        """Build a `TextEntryArray` from a sequence of text bodies and log levels."""
        storage = pa.array(text_entries, type=TextEntryType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(TextEntryArray, pa.ExtensionArray.from_storage(TextEntryType(), storage))
        return storage  # type: ignore[no-any-return]


TextEntryType = ComponentTypeFactory("TextEntryType", TextEntryArray, REGISTERED_COMPONENT_NAMES["rerun.text_entry"])

pa.register_extension_type(TextEntryType())
