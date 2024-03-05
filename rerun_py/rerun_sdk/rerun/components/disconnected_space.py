# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/components/disconnected_space.fbs".

# You can extend this class by creating a "DisconnectedSpaceExt" class in "disconnected_space_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin
from .disconnected_space_ext import DisconnectedSpaceExt

__all__ = [
    "DisconnectedSpace",
    "DisconnectedSpaceArrayLike",
    "DisconnectedSpaceBatch",
    "DisconnectedSpaceLike",
    "DisconnectedSpaceType",
]


@define(init=False)
class DisconnectedSpace(DisconnectedSpaceExt):
    """
    **Component**: Spatially disconnect this entity from its parent.

    Specifies that the entity path at which this is logged is spatially disconnected from its parent,
    making it impossible to transform the entity path into its parent's space and vice versa.
    It *only* applies to space views that work with spatial transformations, i.e. 2D & 3D space views.
    This is useful for specifying that a subgraph is independent of the rest of the scene.
    """

    # __init__ can be found in disconnected_space_ext.py

    def __bool__(self) -> bool:
        return self.is_disconnected

    is_disconnected: bool = field(converter=bool)
    # Whether the entity path at which this is logged is disconnected from its parent.
    #
    # (Docstring intentionally commented out to hide this field from the docs)


if TYPE_CHECKING:
    DisconnectedSpaceLike = Union[DisconnectedSpace, bool]
else:
    DisconnectedSpaceLike = Any

DisconnectedSpaceArrayLike = Union[DisconnectedSpace, Sequence[DisconnectedSpaceLike], bool, npt.NDArray[np.bool_]]


class DisconnectedSpaceType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.components.DisconnectedSpace"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.bool_(), self._TYPE_NAME)


class DisconnectedSpaceBatch(BaseBatch[DisconnectedSpaceArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = DisconnectedSpaceType()

    @staticmethod
    def _native_to_pa_array(data: DisconnectedSpaceArrayLike, data_type: pa.DataType) -> pa.Array:
        return DisconnectedSpaceExt.native_to_pa_array_override(data, data_type)
