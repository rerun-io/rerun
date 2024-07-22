# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/components/latest_at_queries.fbs".

# You can extend this class by creating a "LatestAtQueriesExt" class in "latest_at_queries_ext.py".

from __future__ import annotations

from typing import Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from ..._baseclasses import (
    BaseBatch,
    BaseExtensionType,
    ComponentBatchMixin,
    ComponentMixin,
)
from ...blueprint import datatypes as blueprint_datatypes

__all__ = [
    "LatestAtQueries",
    "LatestAtQueriesArrayLike",
    "LatestAtQueriesBatch",
    "LatestAtQueriesLike",
    "LatestAtQueriesType",
]


@define(init=False)
class LatestAtQueries(ComponentMixin):
    """**Component**: Component(s) used as point-of-view for a query."""

    _BATCH_TYPE = None

    def __init__(self: Any, value: LatestAtQueriesLike):
        """Create a new instance of the LatestAtQueries component."""

        # You can define your own __init__ function as a member of LatestAtQueriesExt in latest_at_queries_ext.py
        self.__attrs_init__(value=value)

    value: list[blueprint_datatypes.LatestAtQuery] = field()


LatestAtQueriesLike = LatestAtQueries
LatestAtQueriesArrayLike = Union[
    LatestAtQueries,
    Sequence[LatestAtQueriesLike],
]


class LatestAtQueriesType(BaseExtensionType):
    _TYPE_NAME: str = "rerun.blueprint.components.LatestAtQueries"

    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.list_(
                pa.field(
                    "item",
                    pa.struct([
                        pa.field("timeline", pa.utf8(), nullable=False, metadata={}),
                        pa.field("time", pa.int64(), nullable=False, metadata={}),
                    ]),
                    nullable=False,
                    metadata={},
                )
            ),
            self._TYPE_NAME,
        )


class LatestAtQueriesBatch(BaseBatch[LatestAtQueriesArrayLike], ComponentBatchMixin):
    _ARROW_TYPE = LatestAtQueriesType()

    @staticmethod
    def _native_to_pa_array(data: LatestAtQueriesArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError(
            "Arrow serialization of LatestAtQueries not implemented: We lack codegen for arrow-serialization of general structs"
        )  # You need to implement native_to_pa_array_override in latest_at_queries_ext.py


# This is patched in late to avoid circular dependencies.
LatestAtQueries._BATCH_TYPE = LatestAtQueriesBatch  # type: ignore[assignment]