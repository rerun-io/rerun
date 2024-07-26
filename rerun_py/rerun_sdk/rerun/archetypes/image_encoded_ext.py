from __future__ import annotations

import pathlib
from io import BytesIO
from typing import IO, TYPE_CHECKING, Any, Union

import numpy as np
import numpy.typing as npt

from rerun.components.color_model import ColorModel, ColorModelLike

from .. import datatypes
from ..error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from ..archetypes import Image, ImageEncoded
    from ..components import MediaType
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


def _to_numpy(tensor: ImageLike) -> npt.NDArray[Any]:
    # isinstance is 4x faster than catching AttributeError
    if isinstance(tensor, np.ndarray):
        return tensor

    try:
        # Make available to the cpu
        return tensor.numpy(force=True)  # type: ignore[union-attr]
    except AttributeError:
        return np.array(tensor, copy=False)


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
        contents: bytes | IO[bytes] | datatypes.BlobLike | None = None,
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

    @staticmethod
    def compress(image: ImageLike, color_model: ColorModelLike, jpeg_quality: int = 95) -> ImageEncoded | Image:
        """
        Compress the given image as a JPEG.

        JPEG compression works best for photographs.
        Only RGB and grayscale images are supported, not RGBA.
        Note that compressing to JPEG costs a bit of CPU time,
        both when logging and later when viewing them.

        Parameters
        ----------
        image:
            The image to compress, as a numpy array or tensor.
        color_model:
            The color model of the image, e.g. "RGB" or "L".
        jpeg_quality:
            Higher quality = larger file size.
            A quality of 95 saves a lot of space, but is still visually very similar.

        """

        from PIL import Image as PILImage

        from ..archetypes import ImageEncoded
        from . import Image

        with catch_and_log_exceptions(context="Image compression"):
            if isinstance(color_model, str):
                color_model = ColorModel[color_model]
            if isinstance(color_model, int):
                color_model = ColorModel(color_model)
            elif not isinstance(color_model, ColorModel):
                raise ValueError(f"Invalid color_model: {color_model}")

            if color_model not in (ColorModel.L, ColorModel.RGB):
                # TODO(#2340): BGR support!
                raise ValueError(
                    f"Cannot JPEG compress an image of type {color_model}. Only L (monochrome) and RGB are supported."
                )

            mode = str(color_model)

            image = _to_numpy(image)
            if image.dtype not in ["uint8", "sint32", "float32"]:
                # Convert to a format supported by Image.fromarray
                image = image.astype("float32")

            pil_image = PILImage.fromarray(image, mode=mode)
            output = BytesIO()
            pil_image.save(output, format="JPEG", quality=jpeg_quality)
            jpeg_bytes = output.getvalue()
            output.close()
            return ImageEncoded(contents=jpeg_bytes, media_type="image/jpeg")

        # On failure to compress, return a raw image
        return Image(image=image, color_model=color_model)
