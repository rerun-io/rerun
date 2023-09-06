from __future__ import annotations

from io import BytesIO
from typing import Any

import numpy as np
import numpy.typing as npt
from PIL import Image

from rerun import bindings
from rerun.log.error_utils import _send_warning
from rerun.log.file import ImageFormat, log_image_file
from rerun.log.log_decorator import log_decorator
from rerun.log.tensor import Tensor, _log_tensor, _to_numpy
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_image",
    "log_depth_image",
    "log_segmentation_image",
]


@log_decorator
def log_image(
    entity_path: str,
    image: Tensor,
    *,
    draw_order: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
    jpeg_quality: int | None = None,
) -> None:
    """
    Log a gray or color image.

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
        A [Tensor][rerun.log.tensor.Tensor] representing the image to log.
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

    recording = RecordingStream.to_native(recording)

    image = _to_numpy(image)

    shape = image.shape
    non_empty_dims = [d for d in shape if d != 1]
    num_non_empty_dims = len(non_empty_dims)

    interpretable_as_image = True
    # Catch some errors early:
    if num_non_empty_dims < 2 or 3 < num_non_empty_dims:
        _send_warning(f"Expected image, got array of shape {shape}", 1, recording=recording)
        interpretable_as_image = False

    if num_non_empty_dims == 3:
        depth = shape[-1]
        if depth not in (1, 3, 4):
            _send_warning(
                f"Expected image depth of 1 (gray), 3 (RGB) or 4 (RGBA). Instead got array of shape {shape}",
                1,
                recording=recording,
            )
            interpretable_as_image = False

    # TODO(#672): Don't squeeze once the image view can handle extra empty dimensions
    if interpretable_as_image and num_non_empty_dims != len(shape):
        image = np.squeeze(image)

    if jpeg_quality is not None:
        # TODO(emilk): encode JPEG in background thread instead

        if image.dtype not in ["uint8", "sint32", "float32"]:
            # Convert to a format supported by Image.fromarray
            image = image.astype("float32")

        pil_image = Image.fromarray(image)
        output = BytesIO()
        pil_image.save(output, format="JPEG", quality=jpeg_quality)
        jpeg_bytes = output.getvalue()
        output.close()

        # TODO(emilk): pass draw_order too
        log_image_file(entity_path=entity_path, img_bytes=jpeg_bytes, img_format=ImageFormat.JPEG, timeless=timeless)
        return

    _log_tensor(entity_path, image, draw_order=draw_order, ext=ext, timeless=timeless, recording=recording)


@log_decorator
def log_depth_image(
    entity_path: str,
    image: Tensor,
    *,
    draw_order: float | None = None,
    meter: float | None = None,
    ext: dict[str, Any] | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log a depth image.

    The image must be a 2D array.

    Supported dtypes
    ----------------
    float16, float32, float64, uint8, uint16, uint32, uint64, int8, int16, int32, int64

    Parameters
    ----------
    entity_path:
        Path to the image in the space hierarchy.
    image:
        A [Tensor][rerun.log.tensor.Tensor] representing the depth image to log.
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

    recording = RecordingStream.to_native(recording)

    image = _to_numpy(image)

    # TODO(#635): Remove when issue with displaying f64 depth images is fixed.
    if image.dtype == np.float64:
        image = image.astype(np.float32)

    shape = image.shape
    non_empty_dims = [d for d in shape if d != 1]
    num_non_empty_dims = len(non_empty_dims)

    # Catch some errors early:
    if num_non_empty_dims != 2:
        _send_warning(f"Expected 2D depth image, got array of shape {shape}", 1, recording=recording)
        _log_tensor(
            entity_path, image, timeless=timeless, meaning=bindings.TensorDataMeaning.Depth, recording=recording
        )
    else:
        # TODO(#672): Don't squeeze once the image view can handle extra empty dimensions.
        if num_non_empty_dims != len(shape):
            image = np.squeeze(image)
        _log_tensor(
            entity_path,
            image,
            draw_order=draw_order,
            meter=meter,
            ext=ext,
            timeless=timeless,
            meaning=bindings.TensorDataMeaning.Depth,
            recording=recording,
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
        A [Tensor][rerun.log.tensor.Tensor] representing the segmentation image to log.
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
    recording = RecordingStream.to_native(recording)

    image = np.array(image, copy=False)
    if image.dtype not in (np.dtype("uint8"), np.dtype("uint16")):
        image = np.require(image, np.uint16)
    non_empty_dims = [d for d in image.shape if d != 1]
    num_non_empty_dims = len(non_empty_dims)

    # Catch some errors early:
    if num_non_empty_dims != 2:
        _send_warning(
            f"Expected single channel image, got array of shape {image.shape}. Can't interpret as segmentation image.",
            1,
            recording=recording,
        )
        _log_tensor(
            entity_path,
            tensor=image,
            draw_order=draw_order,
            ext=ext,
            timeless=timeless,
            recording=recording,
        )
    else:
        # TODO(#672): Don't squeeze once the image view can handle extra empty dimensions.
        if num_non_empty_dims != len(image.shape):
            image = np.squeeze(image)
        _log_tensor(
            entity_path,
            tensor=image,
            draw_order=draw_order,
            meaning=bindings.TensorDataMeaning.ClassId,
            ext=ext,
            timeless=timeless,
            recording=recording,
        )
