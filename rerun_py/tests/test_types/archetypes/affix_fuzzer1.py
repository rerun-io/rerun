# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer1Ext" class in "affix_fuzzer1_ext.py".

from __future__ import annotations

from typing import Any

import numpy as np
import pyarrow as pa
from attrs import define, field
from rerun._baseclasses import (
    Archetype,
    ComponentColumnList,
)
from rerun.error_utils import catch_and_log_exceptions

from .. import components, datatypes

__all__ = ["AffixFuzzer1"]


@define(str=False, repr=False, init=False)
class AffixFuzzer1(Archetype):
    def __init__(
        self: Any,
        fuzz1001: datatypes.AffixFuzzer1Like,
        fuzz1002: datatypes.AffixFuzzer1Like,
        fuzz1003: datatypes.AffixFuzzer1Like,
        fuzz1004: datatypes.AffixFuzzer1Like,
        fuzz1005: datatypes.AffixFuzzer1Like,
        fuzz1006: datatypes.AffixFuzzer1Like,
        fuzz1007: components.AffixFuzzer7Like,
        fuzz1008: components.AffixFuzzer8Like,
        fuzz1009: components.AffixFuzzer9Like,
        fuzz1010: components.AffixFuzzer10Like,
        fuzz1011: components.AffixFuzzer11Like,
        fuzz1012: components.AffixFuzzer12Like,
        fuzz1013: components.AffixFuzzer13Like,
        fuzz1014: datatypes.AffixFuzzer3Like,
        fuzz1015: datatypes.AffixFuzzer3Like,
        fuzz1016: components.AffixFuzzer16Like,
        fuzz1017: components.AffixFuzzer17Like,
        fuzz1018: components.AffixFuzzer18Like,
        fuzz1019: datatypes.AffixFuzzer5Like,
        fuzz1020: datatypes.AffixFuzzer20Like,
        fuzz1021: datatypes.AffixFuzzer21Like,
        fuzz1022: datatypes.AffixFuzzer22Like,
    ):
        """Create a new instance of the AffixFuzzer1 archetype."""

        # You can define your own __init__ function as a member of AffixFuzzer1Ext in affix_fuzzer1_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                fuzz1001=fuzz1001,
                fuzz1002=fuzz1002,
                fuzz1003=fuzz1003,
                fuzz1004=fuzz1004,
                fuzz1005=fuzz1005,
                fuzz1006=fuzz1006,
                fuzz1007=fuzz1007,
                fuzz1008=fuzz1008,
                fuzz1009=fuzz1009,
                fuzz1010=fuzz1010,
                fuzz1011=fuzz1011,
                fuzz1012=fuzz1012,
                fuzz1013=fuzz1013,
                fuzz1014=fuzz1014,
                fuzz1015=fuzz1015,
                fuzz1016=fuzz1016,
                fuzz1017=fuzz1017,
                fuzz1018=fuzz1018,
                fuzz1019=fuzz1019,
                fuzz1020=fuzz1020,
                fuzz1021=fuzz1021,
                fuzz1022=fuzz1022,
            )
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            fuzz1001=None,
            fuzz1002=None,
            fuzz1003=None,
            fuzz1004=None,
            fuzz1005=None,
            fuzz1006=None,
            fuzz1007=None,
            fuzz1008=None,
            fuzz1009=None,
            fuzz1010=None,
            fuzz1011=None,
            fuzz1012=None,
            fuzz1013=None,
            fuzz1014=None,
            fuzz1015=None,
            fuzz1016=None,
            fuzz1017=None,
            fuzz1018=None,
            fuzz1019=None,
            fuzz1020=None,
            fuzz1021=None,
            fuzz1022=None,
        )

    @classmethod
    def _clear(cls) -> AffixFuzzer1:
        """Produce an empty AffixFuzzer1, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        fuzz1001: datatypes.AffixFuzzer1Like | None = None,
        fuzz1002: datatypes.AffixFuzzer1Like | None = None,
        fuzz1003: datatypes.AffixFuzzer1Like | None = None,
        fuzz1004: datatypes.AffixFuzzer1Like | None = None,
        fuzz1005: datatypes.AffixFuzzer1Like | None = None,
        fuzz1006: datatypes.AffixFuzzer1Like | None = None,
        fuzz1007: components.AffixFuzzer7Like | None = None,
        fuzz1008: components.AffixFuzzer8Like | None = None,
        fuzz1009: components.AffixFuzzer9Like | None = None,
        fuzz1010: components.AffixFuzzer10Like | None = None,
        fuzz1011: components.AffixFuzzer11Like | None = None,
        fuzz1012: components.AffixFuzzer12Like | None = None,
        fuzz1013: components.AffixFuzzer13Like | None = None,
        fuzz1014: datatypes.AffixFuzzer3Like | None = None,
        fuzz1015: datatypes.AffixFuzzer3Like | None = None,
        fuzz1016: components.AffixFuzzer16Like | None = None,
        fuzz1017: components.AffixFuzzer17Like | None = None,
        fuzz1018: components.AffixFuzzer18Like | None = None,
        fuzz1019: datatypes.AffixFuzzer5Like | None = None,
        fuzz1020: datatypes.AffixFuzzer20Like | None = None,
        fuzz1021: datatypes.AffixFuzzer21Like | None = None,
        fuzz1022: datatypes.AffixFuzzer22Like | None = None,
    ) -> AffixFuzzer1:
        """Update only some specific fields of a `AffixFuzzer1`."""

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "fuzz1001": fuzz1001,
                "fuzz1002": fuzz1002,
                "fuzz1003": fuzz1003,
                "fuzz1004": fuzz1004,
                "fuzz1005": fuzz1005,
                "fuzz1006": fuzz1006,
                "fuzz1007": fuzz1007,
                "fuzz1008": fuzz1008,
                "fuzz1009": fuzz1009,
                "fuzz1010": fuzz1010,
                "fuzz1011": fuzz1011,
                "fuzz1012": fuzz1012,
                "fuzz1013": fuzz1013,
                "fuzz1014": fuzz1014,
                "fuzz1015": fuzz1015,
                "fuzz1016": fuzz1016,
                "fuzz1017": fuzz1017,
                "fuzz1018": fuzz1018,
                "fuzz1019": fuzz1019,
                "fuzz1020": fuzz1020,
                "fuzz1021": fuzz1021,
                "fuzz1022": fuzz1022,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> AffixFuzzer1:
        """Clear all the fields of a `AffixFuzzer1`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        fuzz1001: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1002: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1003: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1004: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1005: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1006: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1007: components.AffixFuzzer7ArrayLike | None = None,
        fuzz1008: components.AffixFuzzer8ArrayLike | None = None,
        fuzz1009: components.AffixFuzzer9ArrayLike | None = None,
        fuzz1010: components.AffixFuzzer10ArrayLike | None = None,
        fuzz1011: components.AffixFuzzer11ArrayLike | None = None,
        fuzz1012: components.AffixFuzzer12ArrayLike | None = None,
        fuzz1013: components.AffixFuzzer13ArrayLike | None = None,
        fuzz1014: datatypes.AffixFuzzer3ArrayLike | None = None,
        fuzz1015: datatypes.AffixFuzzer3ArrayLike | None = None,
        fuzz1016: components.AffixFuzzer16ArrayLike | None = None,
        fuzz1017: components.AffixFuzzer17ArrayLike | None = None,
        fuzz1018: components.AffixFuzzer18ArrayLike | None = None,
        fuzz1019: datatypes.AffixFuzzer5ArrayLike | None = None,
        fuzz1020: datatypes.AffixFuzzer20ArrayLike | None = None,
        fuzz1021: datatypes.AffixFuzzer21ArrayLike | None = None,
        fuzz1022: datatypes.AffixFuzzer22ArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.
        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                fuzz1001=fuzz1001,
                fuzz1002=fuzz1002,
                fuzz1003=fuzz1003,
                fuzz1004=fuzz1004,
                fuzz1005=fuzz1005,
                fuzz1006=fuzz1006,
                fuzz1007=fuzz1007,
                fuzz1008=fuzz1008,
                fuzz1009=fuzz1009,
                fuzz1010=fuzz1010,
                fuzz1011=fuzz1011,
                fuzz1012=fuzz1012,
                fuzz1013=fuzz1013,
                fuzz1014=fuzz1014,
                fuzz1015=fuzz1015,
                fuzz1016=fuzz1016,
                fuzz1017=fuzz1017,
                fuzz1018=fuzz1018,
                fuzz1019=fuzz1019,
                fuzz1020=fuzz1020,
                fuzz1021=fuzz1021,
                fuzz1022=fuzz1022,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {
            "fuzz1001": fuzz1001,
            "fuzz1002": fuzz1002,
            "fuzz1003": fuzz1003,
            "fuzz1004": fuzz1004,
            "fuzz1005": fuzz1005,
            "fuzz1006": fuzz1006,
            "fuzz1007": fuzz1007,
            "fuzz1008": fuzz1008,
            "fuzz1009": fuzz1009,
            "fuzz1010": fuzz1010,
            "fuzz1011": fuzz1011,
            "fuzz1012": fuzz1012,
            "fuzz1013": fuzz1013,
            "fuzz1014": fuzz1014,
            "fuzz1015": fuzz1015,
            "fuzz1016": fuzz1016,
            "fuzz1017": fuzz1017,
            "fuzz1018": fuzz1018,
            "fuzz1019": fuzz1019,
            "fuzz1020": fuzz1020,
            "fuzz1021": fuzz1021,
            "fuzz1022": fuzz1022,
        }
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[index]
                shape = np.shape(param)  # type: ignore[arg-type]

                batch_length = shape[1] if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]
                num_rows = shape[0] if len(shape) >= 1 else 1  # type: ignore[redundant-expr,misc]
                sizes = batch_length * np.ones(num_rows)
            else:
                # For non-primitive types, default to partitioning each element separately.
                sizes = np.ones(len(arrow_array))

            columns.append(batch.partition(sizes))

        indicator_column = cls.indicator().partition(np.zeros(len(sizes)))
        return ComponentColumnList([indicator_column] + columns)

    fuzz1001: components.AffixFuzzer1Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer1Batch._converter,  # type: ignore[misc]
    )
    fuzz1002: components.AffixFuzzer2Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer2Batch._converter,  # type: ignore[misc]
    )
    fuzz1003: components.AffixFuzzer3Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer3Batch._converter,  # type: ignore[misc]
    )
    fuzz1004: components.AffixFuzzer4Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer4Batch._converter,  # type: ignore[misc]
    )
    fuzz1005: components.AffixFuzzer5Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer5Batch._converter,  # type: ignore[misc]
    )
    fuzz1006: components.AffixFuzzer6Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer6Batch._converter,  # type: ignore[misc]
    )
    fuzz1007: components.AffixFuzzer7Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer7Batch._converter,  # type: ignore[misc]
    )
    fuzz1008: components.AffixFuzzer8Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer8Batch._converter,  # type: ignore[misc]
    )
    fuzz1009: components.AffixFuzzer9Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer9Batch._converter,  # type: ignore[misc]
    )
    fuzz1010: components.AffixFuzzer10Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer10Batch._converter,  # type: ignore[misc]
    )
    fuzz1011: components.AffixFuzzer11Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer11Batch._converter,  # type: ignore[misc]
    )
    fuzz1012: components.AffixFuzzer12Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer12Batch._converter,  # type: ignore[misc]
    )
    fuzz1013: components.AffixFuzzer13Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer13Batch._converter,  # type: ignore[misc]
    )
    fuzz1014: components.AffixFuzzer14Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer14Batch._converter,  # type: ignore[misc]
    )
    fuzz1015: components.AffixFuzzer15Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer15Batch._converter,  # type: ignore[misc]
    )
    fuzz1016: components.AffixFuzzer16Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer16Batch._converter,  # type: ignore[misc]
    )
    fuzz1017: components.AffixFuzzer17Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer17Batch._converter,  # type: ignore[misc]
    )
    fuzz1018: components.AffixFuzzer18Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer18Batch._converter,  # type: ignore[misc]
    )
    fuzz1019: components.AffixFuzzer19Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer19Batch._converter,  # type: ignore[misc]
    )
    fuzz1020: components.AffixFuzzer20Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer20Batch._converter,  # type: ignore[misc]
    )
    fuzz1021: components.AffixFuzzer21Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer21Batch._converter,  # type: ignore[misc]
    )
    fuzz1022: components.AffixFuzzer22Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer22Batch._converter,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
