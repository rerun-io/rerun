from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .. import Vec3D, Vec3DLike


def translationrotationscale3d_translation_converter(data: Vec3DLike | None) -> Vec3D | None:
    if data is None:
        return None
    else:
        from .. import Vec3D

        return Vec3D(data)
