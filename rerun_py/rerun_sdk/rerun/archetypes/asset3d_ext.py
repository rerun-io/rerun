from __future__ import annotations

import pathlib
from typing import Any

from .. import datatypes
from ..error_utils import catch_and_log_exceptions


class Asset3DExt:
    """Extension for [Asset3D][rerun.archetypes.Asset3D]."""

    def __init__(
        self: Any,
        *,
        path: str | pathlib.Path | None = None,
        contents: datatypes.BlobLike | None = None,
        media_type: datatypes.Utf8Like | None = None,
        albedo_factor: datatypes.Rgba32Like | None = None,
    ) -> None:
        """
        Create a new instance of the Asset3D archetype.

        Parameters
        ----------
        path:
            A path to an file stored on the local filesystem. Mutually
            exclusive with `contents`.

        contents:
            The contents of the file. Can be a BufferedReader, BytesIO, or
            bytes. Mutually exclusive with `path`.

        media_type:
            The Media Type of the asset.

            For instance:
             * `model/gltf-binary`
             * `model/gltf+json`
             * `model/obj`
             * `model/stl`

            If omitted, it will be guessed from the `path` (if any),
            or the viewer will try to guess from the contents (magic header).
            If the media type cannot be guessed, the viewer won't be able to render the asset.

        albedo_factor:
            Optional color multiplier for the whole mesh

        """

        from ..components import MediaType

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if (path is None) == (contents is None):
                raise ValueError("Must provide exactly one of 'path' or 'contents'")

            if path is None:
                blob = contents
            else:
                blob = pathlib.Path(path).read_bytes()
                if media_type is None:
                    media_type = MediaType.guess_from_path(path)

            self.__attrs_init__(blob=blob, media_type=media_type, albedo_factor=albedo_factor)
            return

        self.__attrs_clear__()
