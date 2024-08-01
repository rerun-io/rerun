# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/image_encoded.fbs".

# You can extend this class by creating a "ImageEncodedExt" class in "image_encoded_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)
from .image_encoded_ext import ImageEncodedExt

__all__ = ["ImageEncoded"]


@define(str=False, repr=False, init=False)
class ImageEncoded(ImageEncodedExt, Archetype):
    """
    **Archetype**: An image encoded as e.g. a JPEG or PNG.

    Rerun also supports uncompressed images with the [`archetypes.Image`][rerun.archetypes.Image].

    To compress an image, use [`rerun.Image.compress`][].

    Example
    -------
    ### `image_encoded`:
    ```python
    from pathlib import Path

    import rerun as rr

    image_file_path = Path(__file__).parent / "ferris.png"

    rr.init("rerun_example_image_encoded", spawn=True)

    rr.log("image", rr.ImageEncoded(path=image_file_path))
    ```

    """

    # __init__ can be found in image_encoded_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            blob=None,  # type: ignore[arg-type]
            media_type=None,  # type: ignore[arg-type]
            opacity=None,  # type: ignore[arg-type]
            draw_order=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> ImageEncoded:
        """Produce an empty ImageEncoded, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    blob: components.BlobBatch = field(
        metadata={"component": "required"},
        converter=components.BlobBatch._required,  # type: ignore[misc]
    )
    # The encoded content of some image file, e.g. a PNG or JPEG.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    media_type: components.MediaTypeBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.MediaTypeBatch._optional,  # type: ignore[misc]
    )
    # The Media Type of the asset.
    #
    # Supported values:
    # * `image/jpeg`
    # * `image/png`
    #
    # If omitted, the viewer will try to guess from the data blob.
    # If it cannot guess, it won't be able to render the asset.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    opacity: components.OpacityBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.OpacityBatch._optional,  # type: ignore[misc]
    )
    # Opacity of the image, useful for layering several images.
    #
    # Defaults to 1.0 (fully opaque).
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    draw_order: components.DrawOrderBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.DrawOrderBatch._optional,  # type: ignore[misc]
    )
    # An optional floating point value that specifies the 2D drawing order.
    #
    # Objects with higher values are drawn on top of those with lower values.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
