# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer2Ext" class in "affix_fuzzer2_ext.py".

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

__all__ = ["AffixFuzzer2"]


@define(str=False, repr=False, init=False)
class AffixFuzzer2(Archetype):
    def __init__(
        self: Any,
        fuzz1101: datatypes.AffixFuzzer1ArrayLike,
        fuzz1102: datatypes.AffixFuzzer1ArrayLike,
        fuzz1103: datatypes.AffixFuzzer1ArrayLike,
        fuzz1104: datatypes.AffixFuzzer1ArrayLike,
        fuzz1105: datatypes.AffixFuzzer1ArrayLike,
        fuzz1106: datatypes.AffixFuzzer1ArrayLike,
        fuzz1107: components.AffixFuzzer7ArrayLike,
        fuzz1108: components.AffixFuzzer8ArrayLike,
        fuzz1109: components.AffixFuzzer9ArrayLike,
        fuzz1110: components.AffixFuzzer10ArrayLike,
        fuzz1111: components.AffixFuzzer11ArrayLike,
        fuzz1112: components.AffixFuzzer12ArrayLike,
        fuzz1113: components.AffixFuzzer13ArrayLike,
        fuzz1114: datatypes.AffixFuzzer3ArrayLike,
        fuzz1115: datatypes.AffixFuzzer3ArrayLike,
        fuzz1116: components.AffixFuzzer16ArrayLike,
        fuzz1117: components.AffixFuzzer17ArrayLike,
        fuzz1118: components.AffixFuzzer18ArrayLike,
        fuzz1122: datatypes.AffixFuzzer22ArrayLike,
    ):
        """Create a new instance of the AffixFuzzer2 archetype."""

        # You can define your own __init__ function as a member of AffixFuzzer2Ext in affix_fuzzer2_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                fuzz1101=fuzz1101,
                fuzz1102=fuzz1102,
                fuzz1103=fuzz1103,
                fuzz1104=fuzz1104,
                fuzz1105=fuzz1105,
                fuzz1106=fuzz1106,
                fuzz1107=fuzz1107,
                fuzz1108=fuzz1108,
                fuzz1109=fuzz1109,
                fuzz1110=fuzz1110,
                fuzz1111=fuzz1111,
                fuzz1112=fuzz1112,
                fuzz1113=fuzz1113,
                fuzz1114=fuzz1114,
                fuzz1115=fuzz1115,
                fuzz1116=fuzz1116,
                fuzz1117=fuzz1117,
                fuzz1118=fuzz1118,
                fuzz1122=fuzz1122,
            )
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            fuzz1101=None,
            fuzz1102=None,
            fuzz1103=None,
            fuzz1104=None,
            fuzz1105=None,
            fuzz1106=None,
            fuzz1107=None,
            fuzz1108=None,
            fuzz1109=None,
            fuzz1110=None,
            fuzz1111=None,
            fuzz1112=None,
            fuzz1113=None,
            fuzz1114=None,
            fuzz1115=None,
            fuzz1116=None,
            fuzz1117=None,
            fuzz1118=None,
            fuzz1122=None,
        )

    @classmethod
    def _clear(cls) -> AffixFuzzer2:
        """Produce an empty AffixFuzzer2, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        fuzz1101: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1102: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1103: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1104: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1105: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1106: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1107: components.AffixFuzzer7ArrayLike | None = None,
        fuzz1108: components.AffixFuzzer8ArrayLike | None = None,
        fuzz1109: components.AffixFuzzer9ArrayLike | None = None,
        fuzz1110: components.AffixFuzzer10ArrayLike | None = None,
        fuzz1111: components.AffixFuzzer11ArrayLike | None = None,
        fuzz1112: components.AffixFuzzer12ArrayLike | None = None,
        fuzz1113: components.AffixFuzzer13ArrayLike | None = None,
        fuzz1114: datatypes.AffixFuzzer3ArrayLike | None = None,
        fuzz1115: datatypes.AffixFuzzer3ArrayLike | None = None,
        fuzz1116: components.AffixFuzzer16ArrayLike | None = None,
        fuzz1117: components.AffixFuzzer17ArrayLike | None = None,
        fuzz1118: components.AffixFuzzer18ArrayLike | None = None,
        fuzz1122: datatypes.AffixFuzzer22ArrayLike | None = None,
    ) -> AffixFuzzer2:
        """Update only some specific fields of a `AffixFuzzer2`."""

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "fuzz1101": fuzz1101,
                "fuzz1102": fuzz1102,
                "fuzz1103": fuzz1103,
                "fuzz1104": fuzz1104,
                "fuzz1105": fuzz1105,
                "fuzz1106": fuzz1106,
                "fuzz1107": fuzz1107,
                "fuzz1108": fuzz1108,
                "fuzz1109": fuzz1109,
                "fuzz1110": fuzz1110,
                "fuzz1111": fuzz1111,
                "fuzz1112": fuzz1112,
                "fuzz1113": fuzz1113,
                "fuzz1114": fuzz1114,
                "fuzz1115": fuzz1115,
                "fuzz1116": fuzz1116,
                "fuzz1117": fuzz1117,
                "fuzz1118": fuzz1118,
                "fuzz1122": fuzz1122,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> AffixFuzzer2:
        """Clear all the fields of a `AffixFuzzer2`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        fuzz1101: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1102: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1103: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1104: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1105: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1106: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz1107: components.AffixFuzzer7ArrayLike | None = None,
        fuzz1108: components.AffixFuzzer8ArrayLike | None = None,
        fuzz1109: components.AffixFuzzer9ArrayLike | None = None,
        fuzz1110: components.AffixFuzzer10ArrayLike | None = None,
        fuzz1111: components.AffixFuzzer11ArrayLike | None = None,
        fuzz1112: components.AffixFuzzer12ArrayLike | None = None,
        fuzz1113: components.AffixFuzzer13ArrayLike | None = None,
        fuzz1114: datatypes.AffixFuzzer3ArrayLike | None = None,
        fuzz1115: datatypes.AffixFuzzer3ArrayLike | None = None,
        fuzz1116: components.AffixFuzzer16ArrayLike | None = None,
        fuzz1117: components.AffixFuzzer17ArrayLike | None = None,
        fuzz1118: components.AffixFuzzer18ArrayLike | None = None,
        fuzz1122: datatypes.AffixFuzzer22ArrayLike | None = None,
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
                fuzz1101=fuzz1101,
                fuzz1102=fuzz1102,
                fuzz1103=fuzz1103,
                fuzz1104=fuzz1104,
                fuzz1105=fuzz1105,
                fuzz1106=fuzz1106,
                fuzz1107=fuzz1107,
                fuzz1108=fuzz1108,
                fuzz1109=fuzz1109,
                fuzz1110=fuzz1110,
                fuzz1111=fuzz1111,
                fuzz1112=fuzz1112,
                fuzz1113=fuzz1113,
                fuzz1114=fuzz1114,
                fuzz1115=fuzz1115,
                fuzz1116=fuzz1116,
                fuzz1117=fuzz1117,
                fuzz1118=fuzz1118,
                fuzz1122=fuzz1122,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {
            "fuzz1101": fuzz1101,
            "fuzz1102": fuzz1102,
            "fuzz1103": fuzz1103,
            "fuzz1104": fuzz1104,
            "fuzz1105": fuzz1105,
            "fuzz1106": fuzz1106,
            "fuzz1107": fuzz1107,
            "fuzz1108": fuzz1108,
            "fuzz1109": fuzz1109,
            "fuzz1110": fuzz1110,
            "fuzz1111": fuzz1111,
            "fuzz1112": fuzz1112,
            "fuzz1113": fuzz1113,
            "fuzz1114": fuzz1114,
            "fuzz1115": fuzz1115,
            "fuzz1116": fuzz1116,
            "fuzz1117": fuzz1117,
            "fuzz1118": fuzz1118,
            "fuzz1122": fuzz1122,
        }
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[index]
                shape = np.shape(param)  # type: ignore[arg-type]

                batch_length = shape[1] if len(shape) > 1 else 1
                num_rows = shape[0] if len(shape) >= 1 else 1
                sizes = batch_length * np.ones(num_rows)
            else:
                # For non-primitive types, default to partitioning each element separately.
                sizes = np.ones(len(arrow_array))

            columns.append(batch.partition(sizes))

        indicator_column = cls.indicator().partition(np.zeros(len(sizes)))
        return ComponentColumnList([indicator_column] + columns)

    fuzz1101: components.AffixFuzzer1Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer1Batch._converter,  # type: ignore[misc]
    )
    fuzz1102: components.AffixFuzzer2Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer2Batch._converter,  # type: ignore[misc]
    )
    fuzz1103: components.AffixFuzzer3Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer3Batch._converter,  # type: ignore[misc]
    )
    fuzz1104: components.AffixFuzzer4Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer4Batch._converter,  # type: ignore[misc]
    )
    fuzz1105: components.AffixFuzzer5Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer5Batch._converter,  # type: ignore[misc]
    )
    fuzz1106: components.AffixFuzzer6Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer6Batch._converter,  # type: ignore[misc]
    )
    fuzz1107: components.AffixFuzzer7Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer7Batch._converter,  # type: ignore[misc]
    )
    fuzz1108: components.AffixFuzzer8Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer8Batch._converter,  # type: ignore[misc]
    )
    fuzz1109: components.AffixFuzzer9Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer9Batch._converter,  # type: ignore[misc]
    )
    fuzz1110: components.AffixFuzzer10Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer10Batch._converter,  # type: ignore[misc]
    )
    fuzz1111: components.AffixFuzzer11Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer11Batch._converter,  # type: ignore[misc]
    )
    fuzz1112: components.AffixFuzzer12Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer12Batch._converter,  # type: ignore[misc]
    )
    fuzz1113: components.AffixFuzzer13Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer13Batch._converter,  # type: ignore[misc]
    )
    fuzz1114: components.AffixFuzzer14Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer14Batch._converter,  # type: ignore[misc]
    )
    fuzz1115: components.AffixFuzzer15Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer15Batch._converter,  # type: ignore[misc]
    )
    fuzz1116: components.AffixFuzzer16Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer16Batch._converter,  # type: ignore[misc]
    )
    fuzz1117: components.AffixFuzzer17Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer17Batch._converter,  # type: ignore[misc]
    )
    fuzz1118: components.AffixFuzzer18Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer18Batch._converter,  # type: ignore[misc]
    )
    fuzz1122: components.AffixFuzzer22Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer22Batch._converter,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
