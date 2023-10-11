from __future__ import annotations

from io import BytesIO
from typing import TYPE_CHECKING, cast

import numpy as np
import pyarrow as pa

from .._validators import find_non_empty_dim_indices
from ..datatypes import TensorBufferType
from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions

if TYPE_CHECKING:
    from .._image import ImageEncoded
    from ..components import TensorDataBatch
    from ..datatypes import TensorDataArrayLike
    from . import Image


class ImageExt:
    """Extension for [Image][rerun.archetypes.Image]."""

    JPEG_TYPE_ID = list(f.name for f in TensorBufferType().storage_type).index("JPEG")

    def compress(self, *, jpeg_quality: int = 95) -> ImageEncoded | Image:
        """
        Converts an `Image` to an [`rerun.ImageEncoded`][] using JPEG compression.

        JPEG compression works best for photographs. Only RGB or Mono images are
        supported, not RGBA. Note that compressing to JPEG costs a bit of CPU time,
        both when logging and later when viewing them.

        Parameters
        ----------
        jpeg_quality:
            Higher quality = larger file size. A quality of 95 still saves a lot
            of space, but is visually very similar.
        """

        from PIL import Image as PILImage

        from .._image import ImageEncoded
        from . import Image

        self = cast(Image, self)

        with catch_and_log_exceptions(context="Image compression"):
            tensor_data_arrow = self.data.as_arrow_array()

            if tensor_data_arrow[0].value["buffer"].type_code == self.JPEG_TYPE_ID:
                _send_warning_or_raise(
                    "Image is already compressed as JPEG. Ignoring compression request.",
                    1,
                    recording=None,
                )
                return self

            shape_dims = tensor_data_arrow[0].value["shape"].values.field(0).to_numpy()
            non_empty_dims = find_non_empty_dim_indices(shape_dims)
            filtered_shape = shape_dims[non_empty_dims]
            if len(filtered_shape) == 2:
                mode = "L"
            elif len(filtered_shape) == 3 and filtered_shape[-1] == 3:
                mode = "RGB"
            else:
                raise ValueError("Only RGB or Mono images are supported for JPEG compression")

            image_array = tensor_data_arrow[0].value["buffer"].value.values.to_numpy().reshape(filtered_shape)

            if image_array.dtype not in ["uint8", "sint32", "float32"]:
                # Convert to a format supported by Image.fromarray
                image_array = image_array.astype("float32")

            pil_image = PILImage.fromarray(image_array, mode=mode)
            output = BytesIO()
            pil_image.save(output, format="JPEG", quality=jpeg_quality)
            jpeg_bytes = output.getvalue()
            output.close()
            return ImageEncoded(contents=jpeg_bytes)

        # On failure to compress, still return the original image
        return self

    @staticmethod
    @catch_and_log_exceptions("Image converter")
    def data__field_converter_override(data: TensorDataArrayLike) -> TensorDataBatch:
        from ..components import TensorDataBatch
        from ..datatypes import TensorDataType, TensorDimensionType

        tensor_data = TensorDataBatch(data)
        tensor_data_arrow = tensor_data.as_arrow_array()

        tensor_data_type = TensorDataType().storage_type
        shape_data_type = TensorDimensionType().storage_type

        # TODO(jleibs): Doing this on raw arrow data is not great. Clean this up
        # once we coerce to a canonical non-arrow type.
        shape_dims = tensor_data_arrow[0].value["shape"].values.field(0).to_numpy()
        shape_names = tensor_data_arrow[0].value["shape"].values.field(1).to_numpy(zero_copy_only=False)

        non_empty_dims = find_non_empty_dim_indices(shape_dims)

        num_non_empty_dims = len(non_empty_dims)

        # TODO(#3239): What `recording` should we be passing here? How should we be getting it?
        if num_non_empty_dims < 2 or 3 < num_non_empty_dims:
            _send_warning_or_raise(f"Expected image, got array of shape {shape_dims}", 1, recording=None)

        if num_non_empty_dims == 3:
            depth = shape_dims[non_empty_dims[-1]]
            if depth not in (3, 4):
                _send_warning_or_raise(
                    f"Expected image 3 (RGB) or 4 (RGBA). Instead got array of shape {shape_dims}",
                    1,
                    recording=None,
                )

        # IF no labels are set, add them
        # TODO(jleibs): Again, needing to do this at the arrow level is awful
        if all(label is None for label in shape_names):
            for ind, label in zip(non_empty_dims, ["height", "width", "depth"]):
                shape_names[ind] = label

            tensor_data_type = TensorDataType().storage_type
            shape_data_type = TensorDimensionType().storage_type

            shape_names = pa.array(
                shape_names, mask=np.array([n is None for n in shape_names]), type=shape_data_type.field("name").type
            )

            new_shape = pa.ListArray.from_arrays(
                offsets=[0, len(shape_dims)],
                values=pa.StructArray.from_arrays(
                    [
                        tensor_data_arrow[0].value["shape"].values.field(0),
                        shape_names,
                    ],
                    fields=[shape_data_type.field("size"), shape_data_type.field("name")],
                ),
            ).cast(tensor_data_type.field("shape").type)

            return TensorDataBatch(
                pa.StructArray.from_arrays(
                    [
                        new_shape,
                        tensor_data_arrow.storage.field(1),
                    ],
                    fields=[
                        tensor_data_type.field("shape"),
                        tensor_data_type.field("buffer"),
                    ],
                ).cast(tensor_data_arrow.storage.type)
            )
        # TODO(jleibs): Should we enforce specific names on images? Specifically, what if the existing names are wrong.
        return tensor_data
