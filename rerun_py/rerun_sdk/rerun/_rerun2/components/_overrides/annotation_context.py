from __future__ import annotations

from typing import TYPE_CHECKING, Sequence

import pyarrow as pa

if TYPE_CHECKING:
    from ...datatypes import (
        ClassDescriptionMapElem,
        ClassDescriptionMapElemLike,
    )
    from .. import AnnotationContextArrayLike

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


def annotationcontext_native_to_pa_array(data: AnnotationContextArrayLike, data_type: pa.DataType) -> pa.Array:
    from ...datatypes import ClassDescription, ClassDescriptionMapElemArray
    from .. import AnnotationContext

    if isinstance(data, ClassDescription):
        data = [data]

    # TODO(jleibs): Sort out typing on this.
    # We really only want to support AnnotationContext or Sequence[ClassDescriptionMapElemLike]
    # but AnnotationContextArrayLike also allows Sequence[AnnotationContextLike] which we
    # can't really handle. I suspect we need a mono-component attribute to handle this properly.
    if not isinstance(data, AnnotationContext):
        data = AnnotationContext(class_map=data)  # type: ignore[arg-type]

    internal_array = ClassDescriptionMapElemArray.from_similar(data.class_map).storage

    return pa.ListArray.from_arrays(offsets=[0, len(internal_array)], values=internal_array).cast(data_type)
