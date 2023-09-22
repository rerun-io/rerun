# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/arrows3d.fbs".

# You can extend this class by creating a "Arrows3DExt" class in "arrows3d_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)
from .arrows3d_ext import Arrows3DExt

__all__ = ["Arrows3D"]


@define(str=False, repr=False, init=False)
class Arrows3D(Arrows3DExt, Archetype):
    """
    A batch of 3D arrows with optional colors, radii, labels, etc.

    Example
    -------
    ```python
    from math import tau

    import numpy as np
    import rerun as rr
    import rerun.experimental as rr2

    rr.init("rerun_example_arrow3d", spawn=True)

    lengths = np.log2(np.arange(0, 100) + 1)
    angles = np.arange(start=0, stop=tau, step=tau * 0.01)
    vectors = np.column_stack([np.sin(angles) * lengths, np.zeros(100), np.cos(angles) * lengths])
    colors = [[1.0 - c, c, 0.5, 0.5] for c in angles / tau]

    rr2.log("arrows", rr2.Arrows3D(vectors=vectors, colors=colors))
    ```
    """

    # __init__ can be found in arrows3d_ext.py

    vectors: components.Vector3DArray = field(
        metadata={"component": "required"},
        converter=components.Vector3DArray.from_similar,  # type: ignore[misc]
    )
    """
    All the vectors for each arrow in the batch.
    """

    origins: components.Origin3DArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Origin3DArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    All the origin points for each arrow in the batch.
    """

    radii: components.RadiusArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.RadiusArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Optional radii for the arrows.

    The shaft is rendered as a line with `radius = 0.5 * radius`.
    The tip is rendered with `height = 2.0 * radius` and `radius = 1.0 * radius`.
    """

    colors: components.ColorArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Optional colors for the points.
    """

    labels: components.TextArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TextArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Optional text labels for the arrows.
    """

    class_ids: components.ClassIdArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ClassIdArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Optional class Ids for the points.

    The class ID provides colors and labels if not specified explicitly.
    """

    instance_keys: components.InstanceKeyArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.InstanceKeyArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Unique identifiers for each individual point in the batch.
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
