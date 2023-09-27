# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/annotation_context.fbs".

# You can extend this class by creating a "AnnotationContextExt" class in "annotation_context_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components
from .._baseclasses import Archetype

__all__ = ["AnnotationContext"]


@define(str=False, repr=False, init=False)
class AnnotationContext(Archetype):
    """
    The `AnnotationContext` provides additional information on how to display entities.

    Entities can use `ClassId`s and `KeypointId`s to provide annotations, and
    the labels and colors will be looked up in the appropriate
    `AnnotationContext`. We use the *first* annotation context we find in the
    path-hierarchy when searching up through the ancestors of a given entity
    path.

    Examples
    --------
    Rectangles:
    ```python
    import rerun as rr

    rr.init("rerun_example_annotation_context_rects", spawn=True)

    # Log an annotation context to assign a label and color to each class
    rr.log("/", rr.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]))

    # Log a batch of 2 rectangles with different `class_ids`
    rr.log("detections", rr.Boxes2D(mins=[[-2, -2], [0, 0]], sizes=[[3, 3], [2, 2]], class_ids=[1, 2]))

    # Log an extra rect to set the view bounds
    rr.log("bounds", rr.Boxes2D(half_sizes=[2.5, 2.5]))
    ```
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/1200w.png">
      <img src="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/full.png">
    </picture>

    Segmentation:
    ```python
    import numpy as np
    import rerun as rr

    rr.init("rerun_example_annotation_context_segmentation", spawn=True)

    # Create a simple segmentation image
    image = np.zeros((8, 12), dtype=np.uint8)
    image[0:4, 0:6] = 1
    image[4:8, 6:12] = 2

    # Log an annotation context to assign a label and color to each class
    rr.log("segmentation", rr.AnnotationContext([(1, "red", (255, 0, 0)), (2, "green", (0, 255, 0))]))

    rr.log_segmentation_image("segmentation/image", np.array(image))
    ```
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/1200w.png">
      <img src="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/full.png">
    </picture>

    Connections:
    ```python
    import rerun as rr
    from rerun.datatypes import ClassDescription

    rr.init("rerun_example_annotation_context_connections", spawn=True)

    rr.log(
        "/",
        rr.AnnotationContext(
            [
                ClassDescription(
                    info=0,
                    keypoint_annotations=[
                        (0, "zero", (255, 0, 0)),
                        (1, "one", (0, 255, 0)),
                        (2, "two", (0, 0, 255)),
                        (3, "three", (255, 255, 0)),
                    ],
                    keypoint_connections=[(0, 2), (1, 2), (2, 3)],
                )
            ]
        ),
    )

    rr.log(
        "points",
        rr.Points3D(
            [
                (0, 0, 0),
                (50, 0, 20),
                (100, 100, 30),
                (0, 50, 40),
            ],
            class_ids=[0],
            keypoint_ids=[0, 1, 2, 3],
        ),
    )
    ```
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/1200w.png">
      <img src="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/full.png">
    </picture>
    """

    def __init__(self: Any, context: components.AnnotationContextLike):
        """Create a new instance of the AnnotationContext archetype."""

        # You can define your own __init__ function as a member of AnnotationContextExt in annotation_context_ext.py
        self.__attrs_init__(context=context)

    context: components.AnnotationContextBatch = field(
        metadata={"component": "required"},
        converter=components.AnnotationContextBatch,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
