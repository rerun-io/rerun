from __future__ import annotations

import pathlib
from typing import TYPE_CHECKING, Any

from .. import components, datatypes
from ..error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from ..components import MediaType


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
    """Extension for [Asset3D][rerun.archetypes.Asset3D]."""

    def __init__(
        self: Any,
        data: components.BlobLike | str | pathlib.Path,
        *,
        media_type: datatypes.Utf8Like | None = None,
        transform: datatypes.Transform3DLike | None = None,
    ):
        """
        Create a new instance of the Asset3D archetype.

        Parameters
        ----------
        data:
             The asset's data (either a [`rerun.components.Blob`][] or compatible type, including `bytes`, or a file
             path).
        media_type:
             The Media Type of the asset.

             For instance:
             * `model/gltf-binary`
             * `model/obj`

             If omitted, the viewer will try to guess from the data blob.
             If it cannot guess, it won't be able to render the asset.
        transform:
             An out-of-tree transform.

             Applies a transformation to the asset itself without impacting its children.
        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if isinstance(data, (str, pathlib.Path)):
                path = str(data)
                with open(path, "rb") as file:
                    blob: components.BlobLike = file.read()
                if media_type is None:
                    media_type = guess_media_type(path)
            else:
                blob = data

            self.__attrs_init__(blob=blob, media_type=media_type, transform=transform)
            return

        self.__attrs_clear__()
