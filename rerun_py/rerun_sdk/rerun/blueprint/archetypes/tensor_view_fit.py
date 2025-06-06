# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/tensor_view_fit.fbs".

# You can extend this class by creating a "TensorViewFitExt" class in "tensor_view_fit_ext.py".

from __future__ import annotations

from attrs import define, field

from ..._baseclasses import (
    Archetype,
)
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions
from .tensor_view_fit_ext import TensorViewFitExt

__all__ = ["TensorViewFit"]


@define(str=False, repr=False, init=False)
class TensorViewFit(TensorViewFitExt, Archetype):
    """
    **Archetype**: Configures how a selected tensor slice is shown on screen.

    ⚠️ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
    """

    # __init__ can be found in tensor_view_fit_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            scaling=None,
        )

    @classmethod
    def _clear(cls) -> TensorViewFit:
        """Produce an empty TensorViewFit, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        scaling: blueprint_components.ViewFitLike | None = None,
    ) -> TensorViewFit:
        """
        Update only some specific fields of a `TensorViewFit`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        scaling:
            How the image is scaled to fit the view.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "scaling": scaling,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> TensorViewFit:
        """Clear all the fields of a `TensorViewFit`."""
        return cls.from_fields(clear_unset=True)

    scaling: blueprint_components.ViewFitBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=blueprint_components.ViewFitBatch._converter,  # type: ignore[misc]
    )
    # How the image is scaled to fit the view.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
