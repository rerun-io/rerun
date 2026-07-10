from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np

from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions

if TYPE_CHECKING:
    from .. import datatypes


class Ellipses2DExt:
    """Extension for [Ellipses2D][rerun.archetypes.Ellipses2D]."""

    def __init__(
        self: Any,
        *,
        half_sizes: datatypes.Vec2DArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        centers: datatypes.Vec2DArrayLike | None = None,
        line_radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        draw_order: datatypes.Float32ArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the Ellipses2D archetype.

        Parameters
        ----------
        half_sizes:
            All half-extents (semi-axes) that make up the batch of ellipses.
            Specify this instead of `radii`.
        radii:
            All radii that make up this batch of circles.
            Specify this instead of `half_sizes`.
        centers:
            Optional center positions of the ellipses.
        colors:
            Optional colors for the ellipses.
        line_radii:
            Optional radii for the lines that make up the ellipses.
        labels:
            Optional text labels for the ellipses.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        draw_order:
            An optional floating point value that specifies the 2D drawing order.
            Objects with higher values are drawn on top of those with lower values.

            The default for 2D ellipses is 10.0.
        class_ids:
            Optional `ClassId`s for the ellipses.

            The class ID provides colors and labels if not specified explicitly.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if radii is not None:
                if half_sizes is not None:
                    _send_warning_or_raise("Cannot specify both `radii` and `half_sizes` at the same time.", 1)

                radii = np.asarray(radii, dtype=np.float32)
                # Duplicate [r1, r2, ...] to [[r1, r1], [r2, r2], ...]
                half_sizes = np.repeat(np.expand_dims(radii, axis=1), 2, axis=1)

            self.__attrs_init__(
                half_sizes=half_sizes,
                centers=centers,
                line_radii=line_radii,
                colors=colors,
                labels=labels,
                show_labels=show_labels,
                draw_order=draw_order,
                class_ids=class_ids,
            )
            return

        self.__attrs_clear__()
