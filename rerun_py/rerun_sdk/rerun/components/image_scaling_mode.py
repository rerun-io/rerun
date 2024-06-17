# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/image_scaling_mode.fbs".

# You can extend this class by creating a "ImageScalingModeExt" class in "image_scaling_mode_ext.py".

from __future__ import annotations

from typing import Literal, Sequence, Union

import pyarrow as pa

from .._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
)

__all__ = [
    "ImageScalingMode",
    "ImageScalingModeArrayLike",
    "ImageScalingModeBatch",
    "ImageScalingModeLike",
    "ImageScalingModeType",
]


from enum import Enum


class ImageScalingMode(Enum):
    """**Component**: Determines whether an image or texture should be scaled to fit the viewport."""

    Original = 1
    """No scaling, pixel size will match the image's width/height dimensions in pixels."""

    Fill = 2
    """Scale the image for the largest possible fit in the view's container."""

    FillKeepAspectRatio = 3
    """Scale the image for the largest possible fit in the view's container, but keep the original aspect ratio."""


ImageScalingModeLike = Union[ImageScalingMode, Literal["original"] | Literal["fill"] | Literal["fillkeepaspectratio"]]
ImageScalingModeArrayLike = Union[ImageScalingModeLike, Sequence[ImageScalingModeLike]]


class ImageScalingModeType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.ImageScalingMode"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.sparse_union([
                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                pa.field("Original", pa.null(), nullable=True, metadata={}),
                pa.field("Fill", pa.null(), nullable=True, metadata={}),
                pa.field("FillKeepAspectRatio", pa.null(), nullable=True, metadata={}),
            ]),
            self._TYPE_NAME,
        )


class ImageScalingModeBatch(BaseBatch[ImageScalingModeArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = ImageScalingModeType()

    @staticmethod
    def _native_to_pa_array(data: ImageScalingModeArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, (ImageScalingMode, int, str)):
            data = [data]

        types: list[int] = []

        for value in data:
            if value is None:
                types.append(0)
            elif isinstance(value, ImageScalingMode):
                types.append(value.value)  # Actual enum value
            elif isinstance(value, int):
                types.append(value)  # By number
            elif isinstance(value, str):
                if hasattr(ImageScalingMode, value):
                    types.append(ImageScalingMode[value].value)  # fast path
                elif value.lower() == "original":
                    types.append(ImageScalingMode.Original.value)
                elif value.lower() == "fill":
                    types.append(ImageScalingMode.Fill.value)
                elif value.lower() == "fillkeepaspectratio":
                    types.append(ImageScalingMode.FillKeepAspectRatio.value)
                else:
                    raise ValueError(f"Unknown ImageScalingMode kind: {value}")
            else:
                raise ValueError(f"Unknown ImageScalingMode kind: {value}")

        buffers = [
            None,
            pa.array(types, type=pa.int8()).buffers()[1],
        ]
        children = (1 + 3) * [pa.nulls(len(data))]

        return pa.UnionArray.from_buffers(
            type=data_type,
            length=len(data),
            buffers=buffers,
            children=children,
        )
