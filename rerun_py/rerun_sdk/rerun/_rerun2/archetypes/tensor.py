# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)

__all__ = ["Tensor"]


@define(str=False, repr=False)
class Tensor(Archetype):
    """A generic n-dimensional Tensor."""

    data: components.TensorDataArray = field(
        metadata={"component": "primary"},
        converter=components.TensorDataArray.from_similar,  # type: ignore[misc]
    )
    """
    The tensor data
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
