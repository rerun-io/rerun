# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs.

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)

__all__ = ["LineStrips2D"]


@define(str=False, repr=False)
class LineStrips2D(Archetype):
    """
    A batch of line strips with positions and optional colors, radii, labels, etc.

    Example
    -------
    Many strips:
    ```python
    import rerun as rr
    import rerun.experimental as rr2

    rr.init("rerun_example_line_strip2d", spawn=True)

    rr2.log(
       "strips",
       rr2.LineStrips2D(
           [
               [[0, 0], [2, 1], [4, -1], [6, 0]],
               [[0, 3], [1, 4], [2, 2], [3, 4], [4, 2], [5, 4], [6, 3]],
           ],
           colors=[[255, 0, 0], [0, 255, 0]],
           radii=[0.025, 0.005],
           labels=["one strip here", "and one strip there"],
       ),
    )

    # Log an extra rect to set the view bounds
    rr.log_rect("bounds", [3, 1.5, 8, 9], rect_format=rr.RectFormat.XCYCWH)
    ```

    Many individual segments:
    ```python
    import numpy as np
    import rerun as rr
    import rerun.experimental as rr2

    rr.init("rerun_example_line_segments2d", spawn=True)

    rr2.log(
       "segments",
       rr2.LineStrips2D(np.array([[[0, 0], [2, 1]], [[4, -1], [6, 0]]])),
    )

    # TODO(#2786): Rect2D archetype
    # Log an extra rect to set the view bounds
    rr.log_rect("bounds", [3, 0, 8, 6], rect_format=rr.RectFormat.XCYCWH)
    ```
    """

    strips: components.LineStrip2DArray = field(
        metadata={"component": "primary"},
        converter=components.LineStrip2DArray.from_similar,  # type: ignore[misc]
    )
    """
    All the actual 2D line strips that make up the batch.
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

    labels: components.TextArray | None = field(
        metadata={"component": "secondary"},
        default=None,
        converter=components.TextArray.from_similar,  # type: ignore[misc]
    )
    """
    Optional text labels for the line strips.
    """

    draw_order: components.DrawOrderArray | None = field(
        metadata={"component": "secondary"},
        default=None,
        converter=components.DrawOrderArray.from_similar,  # type: ignore[misc]
    )
    """
    An optional floating point value that specifies the 2D drawing order of each line strip.
    Objects with higher values are drawn on top of those with lower values.
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
