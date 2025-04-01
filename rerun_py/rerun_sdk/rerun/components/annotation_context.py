# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/annotation_context.fbs".

# You can extend this class by creating a "AnnotationContextExt" class in "annotation_context_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, Union

import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseBatch,
    ComponentBatchMixin,
    ComponentDescriptor,
    ComponentMixin,
)
from .annotation_context_ext import AnnotationContextExt

__all__ = ["AnnotationContext", "AnnotationContextArrayLike", "AnnotationContextBatch", "AnnotationContextLike"]


@define(init=False)
class AnnotationContext(AnnotationContextExt, ComponentMixin):
    """
    **Component**: The annotation context provides additional information on how to display entities.

    Entities can use [`datatypes.ClassId`][rerun.datatypes.ClassId]s and [`datatypes.KeypointId`][rerun.datatypes.KeypointId]s to provide annotations, and
    the labels and colors will be looked up in the appropriate
    annotation context. We use the *first* annotation context we find in the
    path-hierarchy when searching up through the ancestors of a given entity
    path.

    ⚠️ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
    """

    _BATCH_TYPE = None

    def __init__(self: Any, class_map: AnnotationContextLike) -> None:
        """
        Create a new instance of the AnnotationContext component.

        Parameters
        ----------
        class_map:
            List of class descriptions, mapping class indices to class names, colors etc.

        """

        # You can define your own __init__ function as a member of AnnotationContextExt in annotation_context_ext.py
        self.__attrs_init__(class_map=class_map)

    class_map: list[datatypes.ClassDescriptionMapElem] = field(
        converter=AnnotationContextExt.class_map__field_converter_override,  # type: ignore[misc]
    )
    # List of class descriptions, mapping class indices to class names, colors etc.
    #
    # (Docstring intentionally commented out to hide this field from the docs)


if TYPE_CHECKING:
    AnnotationContextLike = Union[
        AnnotationContext, datatypes.ClassDescriptionArrayLike, Sequence[datatypes.ClassDescriptionMapElemLike]
    ]
else:
    AnnotationContextLike = Any

AnnotationContextArrayLike = Union[
    AnnotationContext,
    Sequence[AnnotationContextLike],
]


class AnnotationContextBatch(BaseBatch[AnnotationContextArrayLike], ComponentBatchMixin):
    _ARROW_DATATYPE = pa.list_(
        pa.field(
            "item",
            pa.struct([
                pa.field("class_id", pa.uint16(), nullable=False, metadata={}),
                pa.field(
                    "class_description",
                    pa.struct([
                        pa.field(
                            "info",
                            pa.struct([
                                pa.field("id", pa.uint16(), nullable=False, metadata={}),
                                pa.field("label", pa.utf8(), nullable=True, metadata={}),
                                pa.field("color", pa.uint32(), nullable=True, metadata={}),
                            ]),
                            nullable=False,
                            metadata={},
                        ),
                        pa.field(
                            "keypoint_annotations",
                            pa.list_(
                                pa.field(
                                    "item",
                                    pa.struct([
                                        pa.field("id", pa.uint16(), nullable=False, metadata={}),
                                        pa.field("label", pa.utf8(), nullable=True, metadata={}),
                                        pa.field("color", pa.uint32(), nullable=True, metadata={}),
                                    ]),
                                    nullable=False,
                                    metadata={},
                                )
                            ),
                            nullable=False,
                            metadata={},
                        ),
                        pa.field(
                            "keypoint_connections",
                            pa.list_(
                                pa.field(
                                    "item",
                                    pa.struct([
                                        pa.field("keypoint0", pa.uint16(), nullable=False, metadata={}),
                                        pa.field("keypoint1", pa.uint16(), nullable=False, metadata={}),
                                    ]),
                                    nullable=False,
                                    metadata={},
                                )
                            ),
                            nullable=False,
                            metadata={},
                        ),
                    ]),
                    nullable=False,
                    metadata={},
                ),
            ]),
            nullable=False,
            metadata={},
        )
    )
    _COMPONENT_DESCRIPTOR: ComponentDescriptor = ComponentDescriptor("rerun.components.AnnotationContext")

    @staticmethod
    def _native_to_pa_array(data: AnnotationContextArrayLike, data_type: pa.DataType) -> pa.Array:
        return AnnotationContextExt.native_to_pa_array_override(data, data_type)


# This is patched in late to avoid circular dependencies.
AnnotationContext._BATCH_TYPE = AnnotationContextBatch  # type: ignore[assignment]
