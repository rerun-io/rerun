# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/datatypes/filter_by_event.fbs".

# You can extend this class by creating a "FilterByEventExt" class in "filter_by_event_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from ... import datatypes
from ..._baseclasses import (
    BaseBatch,
    BaseExtensionType,
)
from ...blueprint import datatypes as blueprint_datatypes
from .filter_by_event_ext import FilterByEventExt

__all__ = ["FilterByEvent", "FilterByEventArrayLike", "FilterByEventBatch", "FilterByEventLike", "FilterByEventType"]


def _filter_by_event__active__special_field_converter_override(x: datatypes.BoolLike) -> datatypes.Bool:
    if isinstance(x, datatypes.Bool):
        return x
    else:
        return datatypes.Bool(x)


@define(init=False)
class FilterByEvent(FilterByEventExt):
    """**Datatype**: Configuration for the filter by event feature of the dataframe view."""

    def __init__(self: Any, active: datatypes.BoolLike, column: blueprint_datatypes.ComponentColumnSelectorLike):
        """
        Create a new instance of the FilterByEvent datatype.

        Parameters
        ----------
        active:
            Whether the filter by event feature is active.
        column:
            The column used when the filter by event feature is used.

        """

        # You can define your own __init__ function as a member of FilterByEventExt in filter_by_event_ext.py
        self.__attrs_init__(active=active, column=column)

    active: datatypes.Bool = field(converter=_filter_by_event__active__special_field_converter_override)
    # Whether the filter by event feature is active.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    column: blueprint_datatypes.ComponentColumnSelector = field()
    # The column used when the filter by event feature is used.
    #
    # (Docstring intentionally commented out to hide this field from the docs)


if TYPE_CHECKING:
    FilterByEventLike = Union[FilterByEvent, str, blueprint_datatypes.ComponentColumnSelector]
else:
    FilterByEventLike = Any

FilterByEventArrayLike = Union[
    FilterByEvent,
    Sequence[FilterByEventLike],
]


class FilterByEventType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.datatypes.FilterByEvent"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct([
                pa.field("active", pa.bool_(), nullable=False, metadata={}),
                pa.field(
                    "column",
                    pa.struct([
                        pa.field("entity_path", pa.utf8(), nullable=False, metadata={}),
                        pa.field("component", pa.utf8(), nullable=False, metadata={}),
                    ]),
                    nullable=False,
                    metadata={},
                ),
            ]),
            self._TYPE_NAME,
        )


class FilterByEventBatch(BaseBatch[FilterByEventArrayLike]):
    _ARROW_TYPE = FilterByEventType()

    @staticmethod
    def _native_to_pa_array(data: FilterByEventArrayLike, data_type: pa.DataType) -> pa.Array:
        return FilterByEventExt.native_to_pa_array_override(data, data_type)
