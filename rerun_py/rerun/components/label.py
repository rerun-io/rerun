from __future__ import annotations

from typing import Sequence, cast
from rerun.components import ComponentTypeFactory, REGISTERED_FIELDS

import pyarrow as pa


class LabelArray(pa.ExtensionArray):  # type: ignore[misc]
    def new(labels: Sequence[str]) -> LabelArray:
        """Build a `LabelArray` from a sequence of str."""

        storage = pa.array(labels, type=LabelType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(LabelArray, pa.ExtensionArray.from_storage(LabelType(), storage))
        return storage  # type: ignore[no-any-return]


LabelType = ComponentTypeFactory("LabelType", LabelArray, REGISTERED_FIELDS["rerun.label"])

pa.register_extension_type(LabelType())
