from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from . import ChannelDatatype, ChannelDatatypeLike, ColorModel, ColorModelLike, PixelFormat, PixelFormatLike


class ImageFormatExt:
    """Extension for [ImageFormat][rerun.datatypes.ImageFormat]."""

    @staticmethod
    def pixel_format__field_converter_override(
        data: PixelFormatLike | None,
    ) -> PixelFormat | None:
        from . import PixelFormat

        if data is None:
            return None

        return PixelFormat.auto(data)

    @staticmethod
    def channel_datatype__field_converter_override(
        data: ChannelDatatypeLike | None,
    ) -> ChannelDatatype | None:
        from . import ChannelDatatype

        if data is None:
            return None

        return ChannelDatatype.auto(data)

    @staticmethod
    def color_model__field_converter_override(
        data: ColorModelLike | None,
    ) -> ColorModel | None:
        from . import ColorModel

        if data is None:
            return None

        return ColorModel.auto(data)
