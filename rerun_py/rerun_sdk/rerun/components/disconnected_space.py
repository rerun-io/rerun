# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
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
    **Component**: Specifies that the entity path at which this is logged is disconnected from its parent.

    This is useful for specifying that a subgraph is independent of the rest of the scene.

    If a transform or pinhole is logged on the same path, this component will be ignored.
    """

    def __init__(self: Any, is_disconnected: DisconnectedSpaceLike):
        """Create a new instance of the DisconnectedSpace component."""

        # You can define your own __init__ function as a member of DisconnectedSpaceExt in disconnected_space_ext.py
        self.__attrs_init__(is_disconnected=is_disconnected)

    is_disconnected: bool = field(converter=bool)


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
