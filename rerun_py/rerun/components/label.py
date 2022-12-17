from __future__ import annotations

from typing import Sequence, cast
from rerun.color_conversion import u8_array_to_rgba
from rerun.components import ComponentTypeFactory, REGISTERED_FIELDS
import numpy.typing as npt
import numpy as np

import pyarrow as pa


class LabelArray(pa.ExtensionArray):  # type: ignore[misc]
    def new(labels: Sequence[str]) -> LabelArray:
        """Build a `LabelArray` from a sequence of str."""

        storage = pa.array(labels, type=LabelType.storage_type)
        return cast(LabelArray, pa.ExtensionArray.from_storage(LabelType(), storage))


LabelType = ComponentTypeFactory("LabelType", LabelArray, REGISTERED_FIELDS["rerun.label"])

pa.register_extension_type(LabelType())
