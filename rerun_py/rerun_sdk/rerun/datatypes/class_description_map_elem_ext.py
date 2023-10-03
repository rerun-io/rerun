from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from . import (
        ClassDescriptionMapElem,
        ClassDescriptionMapElemArrayLike,
        ClassDescriptionMapElemLike,
    )


def _class_description_map_elem_converter(
    data: ClassDescriptionMapElemLike,
) -> ClassDescriptionMapElem:
    from . import ClassDescription, ClassDescriptionMapElem

    if isinstance(data, ClassDescriptionMapElem):
        return data
    elif isinstance(data, ClassDescription):
        return ClassDescriptionMapElem(class_id=data.info.id, class_description=data)
    else:
        desc = ClassDescription(info=data)
        return ClassDescriptionMapElem(class_id=desc.info.id, class_description=desc)


class ClassDescriptionMapElemExt:
    """Extension for [ClassDescriptionMapElem][rerun.datatypes.ClassDescriptionMapElem]."""

    @staticmethod
    def native_to_pa_array_override(data: ClassDescriptionMapElemArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import ClassDescriptionBatch, ClassDescriptionMapElem, ClassIdBatch

        if isinstance(data, ClassDescriptionMapElem):
            data = [data]

        map_items = [_class_description_map_elem_converter(item) for item in data]

        ids = [item.class_id for item in map_items]
        class_descriptions = [item.class_description for item in map_items]

        id_array = ClassIdBatch(ids).as_arrow_array().storage
        desc_array = ClassDescriptionBatch(class_descriptions).as_arrow_array().storage

        return pa.StructArray.from_arrays(
            arrays=[id_array, desc_array],
            fields=[data_type.field("class_id"), data_type.field("class_description")],
        )
