from __future__ import annotations

import pathlib
from typing import TYPE_CHECKING, Any

from .. import datatypes
from ..error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from ..components import MediaType
    from ..datatypes import Float32Like


def guess_media_type(path: str | pathlib.Path) -> MediaType | None:
    from ..components import MediaType

    ext = pathlib.Path(path).suffix.lower()
    if ext == ".jpg" or ext == ".jpeg":
        return MediaType.JPEG
    elif ext == ".png":
        return MediaType.PNG
    else:
        return None


class ImageEncodedExt:
    """Extension for [ImageEncoded][rerun.archetypes.ImageEncoded]."""

    def __init__(
        self: Any,
        *,
        path: str | pathlib.Path | None = None,
        contents: datatypes.BlobLike | None = None,
        media_type: datatypes.Utf8Like | None = None,
        opacity: Float32Like | None = None,
        draw_order: Float32Like | None = None,
    ):
        """
        Create a new instance of the ImageEncoded archetype.

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
             * `image/jpeg`
             * `image/png`

            If omitted, it will be guessed from the `path` (if any),
            or the viewer will try to guess from the contents (magic header).
            If the media type cannot be guessed, the viewer won't be able to render the asset.

        opacity:
            Opacity of the image, useful for layering several images.
            Defaults to 1.0 (fully opaque).

        draw_order:
            An optional floating point value that specifies the 2D drawing
            order. Objects with higher values are drawn on top of those with
            lower values.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if (path is None) == (contents is None):
                raise ValueError("Must provide exactly one of 'path' or 'contents'")

            if path is None:
                blob = contents
            else:
                blob = pathlib.Path(path).read_bytes()
                if media_type is None:
                    media_type = guess_media_type(str(path))

            self.__attrs_init__(blob=blob, media_type=media_type, draw_order=draw_order, opacity=opacity)
            return

        self.__attrs_clear__()
