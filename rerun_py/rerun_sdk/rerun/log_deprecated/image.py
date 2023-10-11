from __future__ import annotations

from typing import Any

import numpy as np
import numpy.typing as npt
from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.any_value import AnyValues
from rerun.archetypes import DepthImage, Image, SegmentationImage
from rerun.datatypes import TensorData
from rerun.datatypes.tensor_data import TensorDataLike
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_image",
    "log_depth_image",
    "log_segmentation_image",
]


@deprecated(
    """Please migrate to `rr.log(…, rr.Image(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_image(
    entity_path: str,
    image: TensorDataLike,
    *,
    draw_order: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
    jpeg_quality: int | None = None,
) -> None:
    """
    Log a gray or color image.

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.Image][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

    The image should either have 1, 3 or 4 channels (gray, RGB or RGBA).

    Supported dtypes
    ----------------
     - uint8, uint16, uint32, uint64: color components should be in 0-`max_uint` sRGB gamma space, except for alpha
       which should be in 0-`max_uint` linear space.
     - float16, float32, float64: all color components should be in 0-1 linear space.
     - int8, int16, int32, int64: if all pixels are positive, they are interpreted as their unsigned counterparts.
       Otherwise, the image is normalized before display (the pixel with the lowest value is black and the pixel with
       the highest value is white).

    Parameters
    ----------
    entity_path:
        Path to the image in the space hierarchy.
    image:
        A [Tensor][rerun.log_deprecated.tensor.Tensor] representing the image to log.
    draw_order:
        An optional floating point value that specifies the 2D drawing order.
        Objects with higher values are drawn on top of those with lower values.
        The default for images is -10.0.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the image will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].
    jpeg_quality:
        If set, encode the image as a JPEG to save storage space.
        Higher quality = larger file size.
        A quality of 95 still saves a lot of space, but is visually very similar.
        JPEG compression works best for photographs.
        Only RGB images are supported.
        Note that compressing to JPEG costs a bit of CPU time, both when logging
        and later when viewing them.

    """

    img = Image(image, draw_order=draw_order)
    if jpeg_quality is not None:
        img = img.compress(jpeg_quality=jpeg_quality)  # type: ignore[assignment]

    log(
        entity_path,
        img,
        AnyValues(**(ext or {})),
        timeless=timeless,
        recording=recording,
    )


@deprecated(
    """Please migrate to `rr.log(…, rr.DepthImage(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_depth_image(
    entity_path: str,
    image: TensorDataLike,
    *,
    draw_order: float | None = None,
    meter: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log a depth image.

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.DepthImage][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

    The image must be a 2D array.

    Supported dtypes
    ----------------
    float16, float32, float64, uint8, uint16, uint32, uint64, int8, int16, int32, int64

    Parameters
    ----------
    entity_path:
        Path to the image in the space hierarchy.
    image:
        A [Tensor][rerun.log_deprecated.tensor.Tensor] representing the depth image to log.
    draw_order:
        An optional floating point value that specifies the 2D drawing order.
        Objects with higher values are drawn on top of those with lower values.
        The default for images is -10.0.
    meter:
        How long is a meter in the given dtype?
        For instance: with uint16, perhaps meter=1000 which would mean
        you have millimeter precision and a range of up to ~65 meters (2^16 / 1000).
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the image will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    tensor_data = TensorData(array=image)

    log(
        entity_path,
        DepthImage(tensor_data, draw_order=draw_order, meter=meter),
        AnyValues(**(ext or {})),
        timeless=timeless,
        recording=recording,
    )


@deprecated(
    """Please migrate to `rr.log(…, rr.SegmentationImage(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_segmentation_image(
    entity_path: str,
    image: npt.ArrayLike,
    *,
    draw_order: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log an image made up of integer class-ids.

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.SegmentationImage][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

    The image should have 1 channel, i.e. be either `H x W` or `H x W x 1`.

    See: [rerun.log_annotation_context][] for information on how to map the class-ids to
    colors and labels.

    Supported dtypes
    ----------------
    uint8, uint16

    Parameters
    ----------
    entity_path:
        Path to the image in the space hierarchy.
    image:
        A [Tensor][rerun.log_deprecated.tensor.Tensor] representing the segmentation image to log.
    draw_order:
        An optional floating point value that specifies the 2D drawing order.
        Objects with higher values are drawn on top of those with lower values.
        The default for images is -10.0.
    ext:
        Optional dictionary of extension components. See [rerun.log_extension_components][]
    timeless:
        If true, the image will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    image = np.asarray(image)
    if image.dtype not in (np.dtype("uint8"), np.dtype("uint16")):
        image = np.require(image, np.uint16)

    tensor_data = TensorData(array=image)

    log(
        entity_path,
        SegmentationImage(tensor_data, draw_order=draw_order),
        AnyValues(**(ext or {})),
        timeless=timeless,
        recording=recording,
    )
