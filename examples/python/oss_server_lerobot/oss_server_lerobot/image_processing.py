"""Image decoding and processing utilities."""

from __future__ import annotations

from io import BytesIO
from typing import TYPE_CHECKING

import numpy as np
import rerun as rr
from PIL import Image

from .utils import unwrap_singleton

if TYPE_CHECKING:
    import pyarrow as pa

    from .types import ImageSpec


def decode_raw_image(buffer_value: object, format_value: object) -> np.ndarray:
    """
    Decode a raw image from buffer and format information.

    Args:
        buffer_value: Raw image buffer data
        format_value: Format specification containing height, width, and color model

    Returns:
        Decoded image as numpy array with shape (height, width, channels)

    Raises:
        ValueError: If buffer or format is missing or invalid

    """
    buffer_value = unwrap_singleton(buffer_value)
    format_value = unwrap_singleton(format_value)
    if buffer_value is None or format_value is None:
        raise ValueError("Missing raw image buffer or format.")

    flattened = np.asarray(buffer_value)
    format_details = format_value
    if isinstance(format_details, dict):
        height = int(format_details["height"])
        width = int(format_details["width"])
        color_model = int(format_details["color_model"])
    else:
        raise ValueError("Raw image format details are missing required fields.")

    num_channels = rr.datatypes.color_model.ColorModel.auto(color_model).num_channels()
    return flattened.reshape(height, width, num_channels)


def decode_compressed_image(blob_value: object) -> np.ndarray:
    """
    Decode a compressed image from a blob.

    Args:
        blob_value: Compressed image data (JPEG, PNG, etc.)

    Returns:
        Decoded image as numpy array

    Raises:
        ValueError: If blob is missing

    """
    blob_value = unwrap_singleton(blob_value)
    if blob_value is None:
        raise ValueError("Missing compressed image blob.")
    # Convert to bytes - handle both bytes-like objects and arrays
    if isinstance(blob_value, bytes):
        blob_bytes = blob_value
    elif isinstance(blob_value, (bytearray, memoryview)):
        blob_bytes = bytes(blob_value)
    elif isinstance(blob_value, np.ndarray):
        blob_bytes = blob_value.tobytes()
    else:
        blob_bytes = bytes(blob_value)  # type: ignore[arg-type]
    image = Image.open(BytesIO(blob_bytes))
    return np.asarray(image)


def infer_image_shape(table: pa.Table, spec: ImageSpec) -> tuple[int, int, int]:
    """
    Infer the shape of images from a sample in the table.

    Args:
        table: PyArrow table containing image data
        spec: Image specification

    Returns:
        Tuple of (height, width, channels)

    Raises:
        ValueError: If image columns are missing or shape cannot be inferred

    """
    if spec.kind == "raw":
        buffer_column = f"{spec.path}:Image:buffer"
        format_column = f"{spec.path}:Image:format"
        if buffer_column not in table.column_names or format_column not in table.column_names:
            raise ValueError(f"Missing raw image columns for {spec.key}.")
        buffers = table[buffer_column].to_pylist()
        formats = table[format_column].to_pylist()
        for buffer_value, format_value in zip(buffers, formats, strict=False):
            if buffer_value is None or format_value is None:
                continue
            decoded = decode_raw_image(buffer_value, format_value)
            return decoded.shape
        raise ValueError(f"Unable to infer raw image shape for {spec.key}.")

    if spec.kind == "compressed":
        blob_column = f"{spec.path}:EncodedImage:blob"
        if blob_column not in table.column_names:
            raise ValueError(f"Missing compressed image column for {spec.key}.")
        blobs = table[blob_column].to_pylist()
        for blob_value in blobs:
            if blob_value is None:
                continue
            decoded = decode_compressed_image(blob_value)
            return decoded.shape
        raise ValueError(f"Unable to infer compressed image shape for {spec.key}.")

    raise ValueError(f"Unsupported image kind '{spec.kind}' for {spec.key}.")
