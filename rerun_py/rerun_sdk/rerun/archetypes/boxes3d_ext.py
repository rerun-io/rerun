from __future__ import annotations

from typing import Any

import numpy as np

from rerun.error_utils import _send_warning

from .. import components, datatypes


class Boxes3DExt:
    def __init__(
        self: Any,
        *,
        sizes: datatypes.Vec3DArrayLike | None = None,
        mins: datatypes.Vec3DArrayLike | None = None,
        half_sizes: datatypes.Vec3DArrayLike | None = None,
        centers: datatypes.Vec3DArrayLike | None = None,
        rotations: datatypes.Rotation3DArrayLike | None = None,
        colors: datatypes.ColorArrayLike | None = None,
        radii: components.RadiusArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        instance_keys: components.InstanceKeyArrayLike | None = None,
    ) -> None:
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
