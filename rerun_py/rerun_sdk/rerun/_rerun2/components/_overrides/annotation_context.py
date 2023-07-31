from __future__ import annotations

from typing import TYPE_CHECKING, Sequence

import numpy as np
import pyarrow as pa

if TYPE_CHECKING:
    from ...datatypes import (
        ClassDescriptionMapElem,
        ClassDescriptionMapElemLike,
    )
    from .. import AnnotationContextLike

################################################################################
# Internal converters
################################################################################


def _class_description_map_elem_converter(
    data: ClassDescriptionMapElemLike,
) -> ClassDescriptionMapElem:
    from ...datatypes import ClassDescription, ClassDescriptionMapElem

    if isinstance(data, ClassDescriptionMapElem):
        return data
    elif isinstance(data, ClassDescription):
        return ClassDescriptionMapElem(class_id=data.info.id, class_description=data)
    else:
        desc = ClassDescription(info=data)
        return ClassDescriptionMapElem(class_id=desc.info.id, class_description=desc)


################################################################################
# Field converters
################################################################################


def annotationcontext_class_map_converter(
    data: Sequence[ClassDescriptionMapElemLike],
) -> list[ClassDescriptionMapElem]:
    return [_class_description_map_elem_converter(item) for item in data]


################################################################################
# Arrow converters
################################################################################


def annotationcontext_native_to_pa_array(data: AnnotationContextLike, data_type: pa.DataType) -> pa.Array:
    from ...datatypes import ClassDescriptionMapElemArray
    from .. import AnnotationContext

    if not isinstance(data, AnnotationContext):
        data = AnnotationContext(class_map=data)

    internal_array = ClassDescriptionMapElemArray.from_similar(data.class_map).storage

    return pa.ListArray.from_arrays(offsets=[0, len(internal_array)], values=internal_array).cast(data_type)
