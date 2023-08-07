# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)

__all__ = ["LineStrips3D"]


@define(str=False, repr=False)
class LineStrips3D(Archetype):
    """
    A batch of line strips with positions and optional colors, radii, labels, etc.

    Example
    -------
    Many strips:
    ```python
    import rerun as rr

    rr.init("line_strip3d", spawn=True)

    rr.log_line_strips_3d(
       "batch",
       [
           [
               [0, 0, 2],
               [1, 0, 2],
               [1, 1, 2],
               [0, 1, 2],
           ],
           [
               [0, 0, 0],
               [0, 0, 1],
               [1, 0, 0],
               [1, 0, 1],
               [1, 1, 0],
               [1, 1, 1],
               [0, 1, 0],
               [0, 1, 1],
           ],
       ],
       colors=[[255, 0, 0], [0, 255, 0]],
       stroke_widths=[0.025, 0.005],
    )
    ```

    Many individual segments:
    ```python
    import rerun as rr

    rr.init("line_segments3d", spawn=True)

    rr.log_line_segments(
       "simple",
       [
           [0, 0, 0],
           [0, 0, 1],
           [1, 0, 0],
           [1, 0, 1],
           [1, 1, 0],
           [1, 1, 1],
           [0, 1, 0],
           [0, 1, 1],
       ],
    )
    ```
    """

    strips: components.LineStrip3DArray = field(
        metadata={"component": "primary"},
        converter=components.LineStrip3DArray.from_similar,  # type: ignore[misc]
    )
    """
    All the actual 3D line strips that make up the batch.
    """

    radii: components.RadiusArray | None = field(
        metadata={"component": "secondary"},
        default=None,
        converter=components.RadiusArray.from_similar,  # type: ignore[misc]
    )
    """
    Optional radii for the line strips.
    """

    colors: components.ColorArray | None = field(
        metadata={"component": "secondary"},
        default=None,
        converter=components.ColorArray.from_similar,  # type: ignore[misc]
    )
    """
    Optional colors for the line strips.
    """

    labels: components.LabelArray | None = field(
        metadata={"component": "secondary"},
        default=None,
        converter=components.LabelArray.from_similar,  # type: ignore[misc]
    )
    """
    Optional text labels for the line strips.
    """

    class_ids: components.ClassIdArray | None = field(
        metadata={"component": "secondary"},
        default=None,
        converter=components.ClassIdArray.from_similar,  # type: ignore[misc]
    )
    """
    Optional `ClassId`s for the lines.

    The class ID provides colors and labels if not specified explicitly.
    """

    instance_keys: components.InstanceKeyArray | None = field(
        metadata={"component": "secondary"},
        default=None,
        converter=components.InstanceKeyArray.from_similar,  # type: ignore[misc]
    )
    """
    Unique identifiers for each individual line strip in the batch.
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
