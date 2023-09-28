from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..components import MediaType
    from . import Asset3D


def guess_media_type(path: str) -> MediaType | None:
    from pathlib import Path

    from ..components import MediaType

    ext = Path(path).suffix
    if ext == ".glb":
        return MediaType.GLB
    elif ext == ".gltf":
        return MediaType.GLTF
    elif ext == ".obj":
        return MediaType.OBJ
    else:
        return None


class Asset3DExt:
    @staticmethod
    def from_file(path: str) -> Asset3D:
        """
        Creates a new [`Asset3D`] from the file contents at `path`.

        The [`MediaType`] will be guessed from the file extension.

        If no [`MediaType`] can be guessed at the moment, the Rerun Viewer will try to guess one
        from the data at render-time. If it can't, rendering will fail with an error.
        """
        from . import Asset3D

        with open(path, "rb") as file:
            return Asset3D.from_bytes(file.read(), guess_media_type(path))

    @staticmethod
    def from_bytes(blob: bytes, media_type: MediaType | None) -> Asset3D:
        """
        Creates a new [`Asset3D`] from the given `bytes`.

        If no [`MediaType`] is specified, the Rerun Viewer will try to guess one from the data
        at render-time. If it can't, rendering will fail with an error.
        """
        from . import Asset3D

        # TODO(cmc): we could try and guess using magic bytes here, like rust does.
        return Asset3D(blob=blob, media_type=media_type)
