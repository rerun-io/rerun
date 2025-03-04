from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from ..datatypes import (
        ClassDescriptionMapElem,
        ClassDescriptionMapElemLike,
    )
    from . import AnnotationContextArrayLike

from ..datatypes.class_description_map_elem_ext import _class_description_map_elem_converter


class AnnotationContextExt:
    """Extension for [AnnotationContext][rerun.components.AnnotationContext]."""

    @staticmethod
    def class_map__field_converter_override(
        data: Sequence[ClassDescriptionMapElemLike],
    ) -> list[ClassDescriptionMapElem]:
        return [_class_description_map_elem_converter(item) for item in data]

    @staticmethod
    def native_to_pa_array_override(data: AnnotationContextArrayLike, data_type: pa.DataType) -> pa.Array:
        from ..datatypes import ClassDescription, ClassDescriptionMapElemBatch
        from . import AnnotationContext

        if isinstance(data, ClassDescription):
            data = [data]

        # TODO(jleibs): Sort out typing on this.
        # We really only want to support AnnotationContext or Sequence[ClassDescriptionMapElemLike]
        # but AnnotationContextArrayLike also allows Sequence[AnnotationContextLike] which we
        # can't really handle. I suspect we need a mono-component attribute to handle this properly.
        if not isinstance(data, AnnotationContext):
            data = AnnotationContext(class_map=data)  # type: ignore[arg-type]

        internal_array = ClassDescriptionMapElemBatch(data.class_map).as_arrow_array()

        return pa.ListArray.from_arrays(offsets=[0, len(internal_array)], values=internal_array).cast(data_type)
