# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)

__all__ = ["AnnotationContext"]


@define(str=False, repr=False)
class AnnotationContext(Archetype):
    """
    The `AnnotationContext` provides additional information on how to display entities.

    Entities can use `ClassId`s and `KeypointId`s to provide annotations, and
    the labels and colors will be looked up in the appropriate
    `AnnotationContext`. We use the *first* annotation context we find in the
    path-hierarchy when searching up through the ancestors of a given entity
    path.

    Example
    -------
    ```python
    import rerun as rr
    import rerun.experimental as rr2

    rr.init("annotation_context_rects", spawn=True)

    # Log an annotation context to assign a label and color to each class
    rr2.log(
       "/",
       rr2.AnnotationContext(
           [
               (1, "red", (255, 0, 0)),
               (2, "green", (0, 255, 0)),
           ]
       ),
    )

    # Log a batch of 2 rectangles with different `class_ids`
    rr.log_rects("/", [[-2, -2, 3, 3], [0, 0, 2, 2]], class_ids=[1, 2], rect_format=rr.RectFormat.XYWH)

    # Log an extra rect to set the view bounds
    rr.log_rect("bounds", [0, 0, 5, 5], rect_format=rr.RectFormat.XCYCWH)
    ```
    """

    context: components.AnnotationContextArray = field(
        metadata={"component": "primary"},
        converter=components.AnnotationContextArray.from_similar,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
