# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/line_strips3d.fbs".

# You can extend this class by creating a "LineStrips3DExt" class in "line_strips3d_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import Archetype

__all__ = ["LineStrips3D"]


@define(str=False, repr=False)
class LineStrips3D(Archetype):
    """
    A batch of line strips with positions and optional colors, radii, labels, etc.

    Examples
    --------
    Many strips:
    ```python
    import rerun as rr

    rr.init("rerun_example_line_strip3d", spawn=True)

    rr.log(
        "strips",
        rr.LineStrips3D(
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
            radii=[0.025, 0.005],
            labels=["one strip here", "and one strip there"],
        ),
    )
    ```

    Many individual segments:
    ```python
    #!/usr/bin/env python3
    import numpy as np
    import rerun as rr

    rr.init("rerun_example_line_segments3d", spawn=True)

    rr.log(
        "segments",
        rr.LineStrips3D(
            np.array(
                [
                    [[0, 0, 0], [0, 0, 1]],
                    [[1, 0, 0], [1, 0, 1]],
                    [[1, 1, 0], [1, 1, 1]],
                    [[0, 1, 0], [0, 1, 1]],
                ],
            )
        ),
    )
    ```
    """

    def __init__(
        self: Any,
        strips: components.LineStrip3DArrayLike,
        radii: components.RadiusArrayLike | None = None,
        colors: datatypes.ColorArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        instance_keys: components.InstanceKeyArrayLike | None = None,
    ):
        # You can define your own __init__ function as a member of LineStrips3DExt in line_strips3d_ext.py
        self.__attrs_init__(
            strips=strips, radii=radii, colors=colors, labels=labels, class_ids=class_ids, instance_keys=instance_keys
        )

    strips: components.LineStrip3DBatch = field(
        metadata={"component": "required"},
        converter=components.LineStrip3DBatch,  # type: ignore[misc]
    )
    """
    All the actual 3D line strips that make up the batch.
    """

    radii: components.RadiusBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.RadiusBatch._optional,  # type: ignore[misc]
    )
    """
    Optional radii for the line strips.
    """

    colors: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    """
    Optional colors for the line strips.
    """

    labels: components.TextBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TextBatch._optional,  # type: ignore[misc]
    )
    """
    Optional text labels for the line strips.
    """

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ClassIdBatch._optional,  # type: ignore[misc]
    )
    """
    Optional `ClassId`s for the lines.

    The class ID provides colors and labels if not specified explicitly.
    """

    instance_keys: components.InstanceKeyBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.InstanceKeyBatch._optional,  # type: ignore[misc]
    )
    """
    Unique identifiers for each individual line strip in the batch.
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
