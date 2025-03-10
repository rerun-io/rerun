# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/tensor_slice_selection.fbs".

# You can extend this class by creating a "TensorSliceSelectionExt" class in "tensor_slice_selection_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from ... import components, datatypes
from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components, datatypes as blueprint_datatypes
from ...error_utils import catch_and_log_exceptions

__all__ = ["TensorSliceSelection"]


@define(str=False, repr=False, init=False)
class TensorSliceSelection(Archetype):
    """**Archetype**: Specifies a 2D slice of a tensor."""

    def __init__(
        self: Any,
        *,
        width: datatypes.TensorDimensionSelectionLike | None = None,
        height: datatypes.TensorDimensionSelectionLike | None = None,
        indices: datatypes.TensorDimensionIndexSelectionArrayLike | None = None,
        slider: blueprint_datatypes.TensorDimensionIndexSliderArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the TensorSliceSelection archetype.

        Parameters
        ----------
        width:
            Which dimension to map to width.

            If not specified, the height will be determined automatically based on the name and index of the dimension.
        height:
            Which dimension to map to height.

            If not specified, the height will be determined automatically based on the name and index of the dimension.
        indices:
            Selected indices for all other dimensions.

            If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
        slider:
            Any dimension listed here will have a slider for the index.

            Edits to the sliders will directly manipulate dimensions on the `indices` list.
            If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
            If not specified, adds slides for any dimension in `indices`.

        """

        # You can define your own __init__ function as a member of TensorSliceSelectionExt in tensor_slice_selection_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(width=width, height=height, indices=indices, slider=slider)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            width=None,
            height=None,
            indices=None,
            slider=None,
        )

    @classmethod
    def _clear(cls) -> TensorSliceSelection:
        """Produce an empty TensorSliceSelection, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        width: datatypes.TensorDimensionSelectionLike | None = None,
        height: datatypes.TensorDimensionSelectionLike | None = None,
        indices: datatypes.TensorDimensionIndexSelectionArrayLike | None = None,
        slider: blueprint_datatypes.TensorDimensionIndexSliderArrayLike | None = None,
    ) -> TensorSliceSelection:
        """
        Update only some specific fields of a `TensorSliceSelection`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        width:
            Which dimension to map to width.

            If not specified, the height will be determined automatically based on the name and index of the dimension.
        height:
            Which dimension to map to height.

            If not specified, the height will be determined automatically based on the name and index of the dimension.
        indices:
            Selected indices for all other dimensions.

            If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
        slider:
            Any dimension listed here will have a slider for the index.

            Edits to the sliders will directly manipulate dimensions on the `indices` list.
            If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
            If not specified, adds slides for any dimension in `indices`.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "width": width,
                "height": height,
                "indices": indices,
                "slider": slider,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> TensorSliceSelection:
        """Clear all the fields of a `TensorSliceSelection`."""
        return cls.from_fields(clear_unset=True)

    width: components.TensorWidthDimensionBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.TensorWidthDimensionBatch._converter,  # type: ignore[misc]
    )
    # Which dimension to map to width.
    #
    # If not specified, the height will be determined automatically based on the name and index of the dimension.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    height: components.TensorHeightDimensionBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.TensorHeightDimensionBatch._converter,  # type: ignore[misc]
    )
    # Which dimension to map to height.
    #
    # If not specified, the height will be determined automatically based on the name and index of the dimension.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    indices: components.TensorDimensionIndexSelectionBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.TensorDimensionIndexSelectionBatch._converter,  # type: ignore[misc]
    )
    # Selected indices for all other dimensions.
    #
    # If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    slider: blueprint_components.TensorDimensionIndexSliderBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=blueprint_components.TensorDimensionIndexSliderBatch._converter,  # type: ignore[misc]
    )
    # Any dimension listed here will have a slider for the index.
    #
    # Edits to the sliders will directly manipulate dimensions on the `indices` list.
    # If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
    # If not specified, adds slides for any dimension in `indices`.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
