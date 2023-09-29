# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/transform3d.fbs".

# You can extend this class by creating a "Transform3DExt" class in "transform3d_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import Archetype
from ..error_utils import catch_and_log_exceptions

__all__ = ["Transform3D"]


@define(str=False, repr=False, init=False)
class Transform3D(Archetype):
    """
    A 3D transform.

    Example
    -------
    ```python
    from math import pi

    import rerun as rr
    from rerun.datatypes import Angle, RotationAxisAngle

    rr.init("rerun_example_transform3d", spawn=True)

    rr.log("base", rr.Arrows3D(origins=[0, 0, 0], vectors=[0, 1, 0]))

    rr.log("base/translated", rr.TranslationAndMat3x3(translation=[1, 0, 0]))
    rr.log("base/translated", rr.Arrows3D(origins=[0, 0, 0], vectors=[0, 1, 0]))

    rr.log(
        "base/rotated_scaled",
        rr.TranslationRotationScale3D(
            rotation=RotationAxisAngle(axis=[0, 0, 1], angle=Angle(rad=pi / 4)),
            scale=2,
        ),
    )
    rr.log("base/rotated_scaled", rr.Arrows3D(origins=[0, 0, 0], vectors=[0, 1, 0]))
    ```
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/1200w.png">
      <img src="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/full.png">
    </picture>
    """

    def __init__(self: Any, transform: datatypes.Transform3DLike):
        """
        Create a new instance of the Transform3D archetype.

        Parameters
        ----------
        transform:
             The transform
        """

        # You can define your own __init__ function as a member of Transform3DExt in transform3d_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(transform=transform)
            return
        self.__attrs_init__()

    transform: components.Transform3DBatch = field(
        metadata={"component": "required"},
        converter=components.Transform3DBatch._required,  # type: ignore[misc]
    )
    """
    The transform
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
