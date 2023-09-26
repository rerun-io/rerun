from __future__ import annotations

from typing import Any

from .. import components, datatypes


class Arrows3DExt:
    def __init__(
        self: Any,
        *,
        vectors: datatypes.Vec3DArrayLike,
        origins: datatypes.Vec3DArrayLike | None = None,
        radii: components.RadiusArrayLike | None = None,
        colors: datatypes.ColorArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        instance_keys: components.InstanceKeyArrayLike | None = None,
    ) -> None:
        # Custom constructor to remove positional arguments and force use of keyword arguments
        # while still making vectors required.
        self.__attrs_init__(
            vectors=vectors,
            origins=origins,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            instance_keys=instance_keys,
        )
