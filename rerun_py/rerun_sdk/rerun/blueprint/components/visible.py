# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/blueprint/components/visible.fbs".

# You can extend this class by creating a "VisibleExt" class in "visible_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from ..._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin
from .visible_ext import VisibleExt

__all__ = ["Visible", "VisibleArrayLike", "VisibleBatch", "VisibleLike", "VisibleType"]


@define(init=False)
class Visible(VisibleExt):
    """**Component**: Whether the container, space view, entity or instance is currently visible."""

    def __init__(self: Any, visible: VisibleLike):
        """Create a new instance of the Visible component."""

        # You can define your own __init__ function as a member of VisibleExt in visible_ext.py
        self.__attrs_init__(visible=visible)

    def __bool__(self) -> bool:
        return self.visible

    visible: bool = field(converter=bool)


if TYPE_CHECKING:
    VisibleLike = Union[Visible, bool]
else:
    VisibleLike = Any

VisibleArrayLike = Union[
    Visible,
    Sequence[VisibleLike],
]


class VisibleType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.components.Visible"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.bool_(), self._TYPE_NAME)


class VisibleBatch(BaseBatch[VisibleArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = VisibleType()

    @staticmethod
    def _native_to_pa_array(data: VisibleArrayLike, data_type: pa.DataType) -> pa.Array:
        return VisibleExt.native_to_pa_array_override(data, data_type)
