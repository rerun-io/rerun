# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/blueprint/components/panel_view.fbs".

# You can extend this class by creating a "PanelViewExt" class in "panel_view_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from ..._baseclasses import BaseBatch, BaseExtensionType, ComponentBatchMixin

__all__ = ["PanelView", "PanelViewArrayLike", "PanelViewBatch", "PanelViewLike", "PanelViewType"]


@define(init=False)
class PanelView:
    """
    **Component**: The state of the panels.

    Unstable. Used for the ongoing blueprint experimentations.
    """

    def __init__(self: Any, is_expanded: PanelViewLike):
        """Create a new instance of the PanelView component."""

        # You can define your own __init__ function as a member of PanelViewExt in panel_view_ext.py
        self.__attrs_init__(is_expanded=is_expanded)

    def __bool__(self) -> bool:
        return self.is_expanded

    is_expanded: bool = field(converter=bool)


PanelViewLike = PanelView
PanelViewArrayLike = Union[
    PanelView,
    Sequence[PanelViewLike],
]


class PanelViewType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.components.PanelView"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.bool_(), self._TYPE_NAME)


class PanelViewBatch(BaseBatch[PanelViewArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = PanelViewType()

    @staticmethod
    def _native_to_pa_array(data: PanelViewArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement native_to_pa_array_override in panel_view_ext.py
