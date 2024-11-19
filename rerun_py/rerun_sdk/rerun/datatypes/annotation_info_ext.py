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

        if isinstance(data, AnnotationInfo):
            data = [data]

        annotations = [ClassDescriptionExt.info__field_converter_override(item) for item in data]

        ids = [item.id for item in annotations]
        labels = [item.label.value if item.label else None for item in annotations]
        colors = [item.color.rgba if item.color else None for item in annotations]

        id_array = pa.array(ids, type=pa.uint16())

        # Note: we can't use from_similar here because we need to handle optional values
        # fortunately these are fairly simple types
        label_array = pa.array(labels, type=Utf8Batch._ARROW_DATATYPE)
        color_array = pa.array(colors, type=Rgba32Batch._ARROW_DATATYPE)

        return pa.StructArray.from_arrays(
            arrays=[id_array, label_array, color_array],
            fields=[data_type.field("id"), data_type.field("label"), data_type.field("color")],
        )
