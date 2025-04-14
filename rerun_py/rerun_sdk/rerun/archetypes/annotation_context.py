# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/annotation_context.fbs".

# You can extend this class by creating a "AnnotationContextExt" class in "annotation_context_ext.py".

from __future__ import annotations

from typing import Any

import numpy as np
import pyarrow as pa
from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
    ComponentColumnList,
)
from ..error_utils import catch_and_log_exceptions

__all__ = ["AnnotationContext"]


@define(str=False, repr=False, init=False)
class AnnotationContext(Archetype):
    """
    **Archetype**: The annotation context provides additional information on how to display entities.

    Entities can use [`components.ClassId`][rerun.components.ClassId]s and [`components.KeypointId`][rerun.components.KeypointId]s to provide annotations, and
    the labels and colors will be looked up in the appropriate
    annotation context. We use the *first* annotation context we find in the
    path-hierarchy when searching up through the ancestors of a given entity
    path.

    See also [`datatypes.ClassDescription`][rerun.datatypes.ClassDescription].

    ⚠️ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**

    Example
    -------
    ### Segmentation:
    ```python
    import numpy as np
    import rerun as rr

    rr.init("rerun_example_annotation_context_segmentation", spawn=True)

    # Create a simple segmentation image
    image = np.zeros((200, 300), dtype=np.uint8)
    image[50:100, 50:120] = 1
    image[100:180, 130:280] = 2

    # Log an annotation context to assign a label and color to each class
    rr.log("segmentation", rr.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]), static=True)

    rr.log("segmentation/image", rr.SegmentationImage(image))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_segmentation/6c9e88fc9d44a08031cadd444c2e58a985cc1208/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_segmentation/6c9e88fc9d44a08031cadd444c2e58a985cc1208/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_segmentation/6c9e88fc9d44a08031cadd444c2e58a985cc1208/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_segmentation/6c9e88fc9d44a08031cadd444c2e58a985cc1208/1200w.png">
      <img src="https://static.rerun.io/annotation_context_segmentation/6c9e88fc9d44a08031cadd444c2e58a985cc1208/full.png" width="640">
    </picture>
    </center>

    """

    def __init__(self: Any, context: components.AnnotationContextLike) -> None:
        """
        Create a new instance of the AnnotationContext archetype.

        Parameters
        ----------
        context:
            List of class descriptions, mapping class indices to class names, colors etc.

        """

        # You can define your own __init__ function as a member of AnnotationContextExt in annotation_context_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(context=context)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            context=None,
        )

    @classmethod
    def _clear(cls) -> AnnotationContext:
        """Produce an empty AnnotationContext, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        context: components.AnnotationContextLike | None = None,
    ) -> AnnotationContext:
        """
        Update only some specific fields of a `AnnotationContext`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        context:
            List of class descriptions, mapping class indices to class names, colors etc.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "context": context,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> AnnotationContext:
        """Clear all the fields of a `AnnotationContext`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        context: components.AnnotationContextArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        context:
            List of class descriptions, mapping class indices to class names, colors etc.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                context=context,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {"context": context}
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays and fixed size list arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type) or pa.types.is_fixed_size_list(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[index]
                shape = np.shape(param)  # type: ignore[arg-type]
                elem_flat_len = int(np.prod(shape[1:])) if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                if pa.types.is_fixed_size_list(arrow_array.type) and arrow_array.type.list_size == elem_flat_len:
                    # If the product of the last dimensions of the shape are equal to the size of the fixed size list array,
                    # we have `num_rows` single element batches (each element is a fixed sized list).
                    # (This should have been already validated by conversion to the arrow_array)
                    batch_length = 1
                else:
                    batch_length = int(np.prod(shape[1:])) if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                num_rows = shape[0] if len(shape) >= 1 else 1  # type: ignore[redundant-expr,misc]
                sizes = batch_length * np.ones(num_rows)
            else:
                # For non-primitive types, default to partitioning each element separately.
                sizes = np.ones(len(arrow_array))

            columns.append(batch.partition(sizes))

        indicator_column = cls.indicator().partition(np.zeros(len(sizes)))
        return ComponentColumnList([indicator_column] + columns)

    context: components.AnnotationContextBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AnnotationContextBatch._converter,  # type: ignore[misc]
    )
    # List of class descriptions, mapping class indices to class names, colors etc.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
