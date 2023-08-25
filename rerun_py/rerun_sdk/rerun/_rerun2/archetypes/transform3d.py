# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)

__all__ = ["Transform3D"]


@define(str=False, repr=False)
class Transform3D(Archetype):
    """
    A 3D transform.

    Example
    -------
    ```python
    import rerun as rr
    import rerun.experimental as rr2
    from rerun.experimental import dt as rrd

    rr.init("rerun-example-transform3d", spawn=True)

    origin = [0, 0, 0]
    base_vector = [0, 1, 0]

    rr.log_arrow("base", origin=origin, vector=base_vector)

    rr2.log("base/translated", rr2.Transform3D(rrd.TranslationRotationScale3D(translation=[1, 0, 0])))

    rr.log_arrow("base/translated", origin=origin, vector=base_vector)

    rr2.log(
       "base/rotated_scaled",
       rrd.TranslationRotationScale3D(
           rotation=rrd.RotationAxisAngle(axis=[0, 0, 1], radians=3.14 / 4), scale=rrd.Scale3D(2)
       ),
    )

    rr.log_arrow("base/rotated_scaled", origin=origin, vector=base_vector)
    ```
    """

    transform: components.Transform3DArray = field(
        metadata={"component": "primary"},
        converter=components.Transform3DArray.from_similar,  # type: ignore[misc]
    )
    """
    The transform
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
