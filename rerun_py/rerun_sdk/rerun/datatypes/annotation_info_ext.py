from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

from .class_description_ext import ClassDescriptionExt

if TYPE_CHECKING:
    from . import (
        AnnotationInfoArrayLike,
    )


class AnnotationInfoExt:
    """Extension for [AnnotationInfo][rerun.datatypes.AnnotationInfo]."""

    @staticmethod
    def native_to_pa_array_override(data: AnnotationInfoArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import AnnotationInfo, Rgba32Batch, Utf8Batch

        _ = data_type  # unused

        if isinstance(data, AnnotationInfo):
            data = [data]

        annotations = [ClassDescriptionExt.info__field_converter_override(item) for item in data]

        ids = [item.id for item in annotations]
        labels = [item.label.value if item.label else None for item in annotations]
        colors = [item.color.rgba if item.color else None for item in annotations]

        # Nullable fields (label, color) must stay as pa.array() since they can contain None
        return {
            "id": pa.array(ids, type=pa.uint16()),
            "label": pa.array(labels, type=Utf8Batch._ARROW_DATATYPE),
            "color": pa.array(colors, type=Rgba32Batch._ARROW_DATATYPE),
        }
