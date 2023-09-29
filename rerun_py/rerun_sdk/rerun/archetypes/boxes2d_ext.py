from __future__ import annotations

from typing import Any

import numpy as np

from .. import components, datatypes
from ..error_utils import _send_warning, catch_and_log_exceptions


class Boxes2DExt:
    def __init__(
        self: Any,
        *,
        sizes: datatypes.Vec2DArrayLike | None = None,
        mins: datatypes.Vec2DArrayLike | None = None,
        half_sizes: datatypes.Vec2DArrayLike | None = None,
        centers: datatypes.Vec2DArrayLike | None = None,
        radii: components.RadiusArrayLike | None = None,
        colors: datatypes.ColorArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        draw_order: components.DrawOrderLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        instance_keys: components.InstanceKeyArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the Boxes2D archetype.

        Parameters
        ----------
        sizes:
            Full extents in x/y. Specify this instead of `half_sizes`
        half_sizes:
            All half-extents that make up the batch of boxes. Specify this instead of `sizes`
        mins:
            Minimum coordinates of the boxes. Specify this instead of `centers`.

            Only valid when used together with either `sizes` or `half_sizes`.
        centers:
            Optional center positions of the boxes.
        colors:
            Optional colors for the boxes.
        radii:
            Optional radii for the lines that make up the boxes.
        labels:
            Optional text labels for the boxes.
        draw_order:
            An optional floating point value that specifies the 2D drawing order.
            Objects with higher values are drawn on top of those with lower values.

            The default for 2D boxes is 10.0.
        class_ids:
            Optional `ClassId`s for the boxes.

            The class ID provides colors and labels if not specified explicitly.
        instance_keys:
            Unique identifiers for each individual boxes in the batch.
        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if sizes is not None:
                if half_sizes is not None:
                    _send_warning("Cannot specify both `sizes` and `half_sizes` at the same time.", 1)

                sizes = np.asarray(sizes, dtype=np.float32)
                half_sizes = sizes / 2.0

            if mins is not None:
                if centers is not None:
                    _send_warning("Cannot specify both `mins` and `centers` at the same time.", 1)

                # already converted `sizes` to `half_sizes`
                if half_sizes is None:
                    _send_warning("Cannot specify `mins` without `sizes` or `half_sizes`.", 1)
                    half_sizes = np.asarray([1, 1], dtype=np.float32)

                mins = np.asarray(mins, dtype=np.float32)
                half_sizes = np.asarray(half_sizes, dtype=np.float32)
                centers = mins + half_sizes

            self.__attrs_init__(
                half_sizes=half_sizes,
                centers=centers,
                radii=radii,
                colors=colors,
                labels=labels,
                draw_order=draw_order,
                class_ids=class_ids,
                instance_keys=instance_keys,
            )
            return

        self.__attrs_init__(
            half_sizes=None,
            centers=None,
            radii=None,
            colors=None,
            labels=None,
            draw_order=None,
            class_ids=None,
            instance_keys=None,
        )
