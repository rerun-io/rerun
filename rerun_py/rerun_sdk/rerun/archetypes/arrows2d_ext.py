from __future__ import annotations

from typing import Any

from .. import datatypes
from ..error_utils import catch_and_log_exceptions


class Arrows2DExt:
    """Extension for [Arrows2D][rerun.archetypes.Arrows2D]."""

    def __init__(
        self: Any,
        *,
        vectors: datatypes.Vec2DArrayLike,
        origins: datatypes.Vec2DArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the Arrows2D archetype.

        Parameters
        ----------
        vectors:
            All the vectors for each arrow in the batch.
        origins:
            All the origin points for each arrow in the batch.

            If no origins are set, (0, 0, 0) is used as the origin for each arrow.
        radii:
            Optional radii for the arrows.

            The shaft is rendered as a line with `radius = 0.5 * radius`.
            The tip is rendered with `height = 2.0 * radius` and `radius = 1.0 * radius`.
        colors:
            Optional colors for the points.
        labels:
            Optional text labels for the arrows.
        class_ids:
            Optional class Ids for the points.

            The class ID provides colors and labels if not specified explicitly.

        """

        # Custom constructor to remove positional arguments and force use of keyword arguments
        # while still making vectors required.
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                vectors=vectors,
                origins=origins,
                radii=radii,
                colors=colors,
                labels=labels,
                class_ids=class_ids,
            )
            return
        self.__attrs_clear__()
