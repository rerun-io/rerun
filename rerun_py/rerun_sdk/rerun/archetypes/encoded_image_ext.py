from __future__ import annotations

import pathlib
from typing import IO, TYPE_CHECKING, Any, Union

import numpy as np
import numpy.typing as npt

from .. import datatypes
from ..error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from ..datatypes import Float32Like

    ImageLike = Union[
        npt.NDArray[np.float16],
        npt.NDArray[np.float32],
        npt.NDArray[np.float64],
        npt.NDArray[np.int16],
        npt.NDArray[np.int32],
        npt.NDArray[np.int64],
        npt.NDArray[np.int8],
        npt.NDArray[np.uint16],
        npt.NDArray[np.uint32],
        npt.NDArray[np.uint64],
        npt.NDArray[np.uint8],
    ]


class EncodedImageExt:
    """Extension for [EncodedImage][rerun.archetypes.EncodedImage]."""

    def __init__(
        self: Any,
        *,
        path: str | pathlib.Path | None = None,
        contents: bytes | IO[bytes] | datatypes.BlobLike | None = None,
        media_type: datatypes.Utf8Like | None = None,
        opacity: Float32Like | None = None,
        draw_order: Float32Like | None = None,
    ) -> None:
        """
        Create a new instance of the EncodedImage archetype.

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

        from ..components import MediaType

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if (path is None) == (contents is None):
                raise ValueError("Must provide exactly one of 'path' or 'contents'")

            if path is None:
                blob = contents

                if media_type is None:
                    raise ValueError("Must provide 'media_type' when 'contents' is provided")
            else:
                blob = pathlib.Path(path).read_bytes()

                if media_type is None:
                    media_type = MediaType.guess_from_path(path)

            self.__attrs_init__(blob=blob, media_type=media_type, draw_order=draw_order, opacity=opacity)
            return

        self.__attrs_clear__()
