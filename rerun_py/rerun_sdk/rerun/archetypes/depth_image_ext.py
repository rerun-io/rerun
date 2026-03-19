from __future__ import annotations

from io import BytesIO
from typing import TYPE_CHECKING, Any

import numpy as np
import numpy.typing as npt
from PIL import Image as PILImage

from ..components import ColormapLike, ImageFormat
from ..datatypes import ChannelDatatype, Float32Like
from ..error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from rerun.datatypes.range1d import Range1DLike

    from .. import components
    from . import DepthImage, EncodedDepthImage

    ImageLike = (
        npt.NDArray[np.float16]
        | npt.NDArray[np.float32]
        | npt.NDArray[np.float64]
        | npt.NDArray[np.int16]
        | npt.NDArray[np.int32]
        | npt.NDArray[np.int64]
        | npt.NDArray[np.int8]
        | npt.NDArray[np.uint16]
        | npt.NDArray[np.uint32]
        | npt.NDArray[np.uint64]
        | npt.NDArray[np.uint8]
        | np.ndarray[Any, np.dtype[np.floating | np.integer]]
    )


def _as_arrow_or_none(batch: Any) -> Any:
    """Extract arrow array from a batch, or return None."""
    return batch.as_arrow_array() if batch is not None else None


def _to_numpy(tensor: ImageLike) -> npt.NDArray[Any]:
    # isinstance is 4x faster than catching AttributeError
    if isinstance(tensor, np.ndarray):
        return tensor

    try:
        # Make available to the cpu
        return tensor.numpy(force=True)
    except AttributeError:
        return np.asarray(tensor)


class DepthImageExt:
    """Extension for [DepthImage][rerun.archetypes.DepthImage]."""

    def __init__(
        self: Any,
        image: ImageLike,
        *,
        meter: Float32Like | None = None,
        colormap: ColormapLike | None = None,
        depth_range: Range1DLike | None = None,
        point_fill_ratio: Float32Like | None = None,
        draw_order: Float32Like | None = None,
        magnification_filter: components.MagnificationFilterLike | None = None,
    ) -> None:
        """
        Create a new instance of the DepthImage archetype.

        Parameters
        ----------
        image:
            A numpy array or tensor with the depth image data.
            Leading and trailing unit-dimensions are ignored, so that
            `1x480x640x1` is treated as a `480x640`.
        meter:
            An optional floating point value that specifies how long a meter is in the native depth units.

            For instance: with uint16, perhaps meter=1000 which would mean you have millimeter precision
            and a range of up to ~65 meters (2^16 / 1000).

            Note that the only effect on 2D views is the physical depth values shown when hovering the image.
            In 3D views on the other hand, this affects where the points of the point cloud are placed.
        colormap:
            Colormap to use for rendering the depth image.

            If not set, the depth image will be rendered using the Turbo colormap.
        depth_range:
            The expected range of depth values.

            This is typically the expected range of valid values.
            Everything outside of the range is clamped to the range for the purpose of colormpaping.
            Note that point clouds generated from this image will still display all points, regardless of this range.

            If not specified, the range will be automatically be estimated from the data.
            Note that the Viewer may try to guess a wider range than the minimum/maximum of values
            in the contents of the depth image.
            E.g. if all values are positive, some bigger than 1.0 and all smaller than 255.0,
            the Viewer will guess that the data likely came from an 8bit image, thus assuming a range of 0-255.
        point_fill_ratio:
            Scale the radii of the points in the point cloud generated from this image.

            A fill ratio of 1.0 (the default) means that each point is as big as to touch the center of its neighbor
            if it is at the same depth, leaving no gaps.
            A fill ratio of 0.5 means that each point touches the edge of its neighbor if it has the same depth.

            TODO(#6744): This applies only to 3D views!
        draw_order:
            An optional floating point value that specifies the 2D drawing order, used only if the depth image is shown as a 2D image.

            Objects with higher values are drawn on top of those with lower values.
        magnification_filter:
            Optional filter used when a texel is magnified (displayed larger than a screen pixel) in 2D views.

        """
        image = _to_numpy(image)

        shape = image.shape

        # Ignore leading and trailing dimensions of size 1:
        while 2 < len(shape) and shape[0] == 1:
            shape = shape[1:]
        while 2 < len(shape) and shape[-1] == 1:
            shape = shape[:-1]

        if len(shape) != 2:
            raise ValueError(f"DepthImage must be 2D, got shape {image.shape}")
        height, width = shape

        try:
            datatype = ChannelDatatype.from_np_dtype(image.dtype)
        except KeyError:
            raise ValueError(f"Unsupported dtype {image.dtype} for DepthImage") from None

        self.__attrs_init__(
            buffer=image.tobytes(),
            format=ImageFormat(
                width=width,
                height=height,
                channel_datatype=datatype,
            ),
            meter=meter,
            colormap=colormap,
            depth_range=depth_range,
            point_fill_ratio=point_fill_ratio,
            draw_order=draw_order,
            magnification_filter=magnification_filter,
        )

    def image_format(self: Any) -> ImageFormat:
        """Returns the image format of this depth image."""
        image_format_arrow = self.format.as_arrow_array()[0].as_py()
        return ImageFormat(
            width=image_format_arrow["width"],
            height=image_format_arrow["height"],
            channel_datatype=image_format_arrow["channel_datatype"],
        )

    def as_pil_image(self: Any) -> PILImage.Image:
        """Convert the depth image to a PIL Image."""
        image_format = self.image_format()

        np_dtype = image_format.channel_datatype.to_np_dtype()
        buf = self.buffer.as_arrow_array().values.to_numpy().view(np_dtype)
        image_array = buf.reshape(image_format.height, image_format.width)

        return PILImage.fromarray(image_array)

    def compress(self: Any, compress_level: int = 6) -> EncodedDepthImage | DepthImage:
        """
        Compress the given depth image as a PNG.

        PNG compression is lossless. Only U8 and U16 depth images are supported,
        as these are the only single-channel types the Rerun Viewer can decode
        from encoded depth PNGs.

        Note that compressing to PNG costs a bit of CPU time,
        both when logging and later when viewing them.

        Parameters
        ----------
        compress_level:
            PNG compression level, 0-9. Higher means smaller files but slower.
            0 = no compression, 1 = fastest, 9 = smallest. Default is 6.

        """

        from ..archetypes import EncodedDepthImage

        with catch_and_log_exceptions(context="DepthImage compression"):
            if self.format is None:
                raise ValueError("Cannot PNG compress a depth image without a known image_format")

            if self.buffer is None:
                raise ValueError("Cannot PNG compress a depth image without data")

            image_format = self.image_format()

            if image_format.channel_datatype not in (ChannelDatatype.U8, ChannelDatatype.U16):
                raise ValueError(
                    f"Cannot PNG compress a depth image of datatype {image_format.channel_datatype}. "
                    "Only U8 and U16 are supported.",
                )

            pil_image = self.as_pil_image()

            output = BytesIO()
            pil_image.save(output, format="PNG", compress_level=compress_level)
            png_bytes = output.getvalue()
            output.close()

            return EncodedDepthImage(
                blob=png_bytes,
                media_type="image/png",
                meter=_as_arrow_or_none(self.meter),
                colormap=_as_arrow_or_none(self.colormap),
                depth_range=_as_arrow_or_none(self.depth_range),
                point_fill_ratio=_as_arrow_or_none(self.point_fill_ratio),
                draw_order=_as_arrow_or_none(self.draw_order),
            )

        # On failure to compress, return the raw depth image
        return self  # type: ignore[no-any-return]
