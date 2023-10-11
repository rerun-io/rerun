from __future__ import annotations

from typing import Any

import numpy as np

from .. import components, datatypes
from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions


class Boxes3DExt:
    """Extension for [Boxes3D][rerun.archetypes.Boxes3D]."""

    def __init__(
        self: Any,
        *,
        sizes: datatypes.Vec3DArrayLike | None = None,
        mins: datatypes.Vec3DArrayLike | None = None,
        half_sizes: datatypes.Vec3DArrayLike | None = None,
        centers: datatypes.Vec3DArrayLike | None = None,
        rotations: datatypes.Rotation3DArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        radii: components.RadiusArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        instance_keys: components.InstanceKeyArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the Boxes3D archetype.

        Parameters
        ----------
        sizes:
            Full extents in x/y/z. Specify this instead of `half_sizes`
        half_sizes:
            All half-extents that make up the batch of boxes. Specify this instead of `sizes`
        mins:
            Minimum coordinates of the boxes. Specify this instead of `centers`.

            Only valid when used together with either `sizes` or `half_sizes`.
        centers:
            Optional center positions of the boxes.
        rotations:
            Optional rotations of the boxes.
        colors:
            Optional colors for the boxes.
        radii:
            Optional radii for the lines that make up the boxes.
        labels:
            Optional text labels for the boxes.
        class_ids:
            Optional `ClassId`s for the boxes.

            The class ID provides colors and labels if not specified explicitly.
        instance_keys:
            Unique identifiers for each individual boxes in the batch.
        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if sizes is not None:
                if half_sizes is not None:
                    _send_warning_or_raise("Cannot specify both `sizes` and `half_sizes` at the same time.", 1)

                sizes = np.asarray(sizes, dtype=np.float32)
                half_sizes = sizes / 2.0

            if mins is not None:
                if centers is not None:
                    _send_warning_or_raise("Cannot specify both `mins` and `centers` at the same time.", 1)

                # already converted `sizes` to `half_sizes`
                if half_sizes is None:
                    _send_warning_or_raise("Cannot specify `mins` without `sizes` or `half_sizes`.", 1)
                    half_sizes = np.asarray([1, 1, 1], dtype=np.float32)

                mins = np.asarray(mins, dtype=np.float32)
                half_sizes = np.asarray(half_sizes, dtype=np.float32)
                centers = mins + half_sizes

            self.__attrs_init__(
                half_sizes=half_sizes,
                centers=centers,
                rotations=rotations,
                colors=colors,
                radii=radii,
                labels=labels,
                class_ids=class_ids,
                instance_keys=instance_keys,
            )
            return

        self.__attrs_clear__()
