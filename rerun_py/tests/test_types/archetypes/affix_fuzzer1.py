# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer1Ext" class in "affix_fuzzer1_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field
from rerun._baseclasses import (
    Archetype,
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
            fuzz1001=None,  # type: ignore[arg-type]
            fuzz1002=None,  # type: ignore[arg-type]
            fuzz1003=None,  # type: ignore[arg-type]
            fuzz1004=None,  # type: ignore[arg-type]
            fuzz1005=None,  # type: ignore[arg-type]
            fuzz1006=None,  # type: ignore[arg-type]
            fuzz1007=None,  # type: ignore[arg-type]
            fuzz1008=None,  # type: ignore[arg-type]
            fuzz1009=None,  # type: ignore[arg-type]
            fuzz1010=None,  # type: ignore[arg-type]
            fuzz1011=None,  # type: ignore[arg-type]
            fuzz1012=None,  # type: ignore[arg-type]
            fuzz1013=None,  # type: ignore[arg-type]
            fuzz1014=None,  # type: ignore[arg-type]
            fuzz1015=None,  # type: ignore[arg-type]
            fuzz1016=None,  # type: ignore[arg-type]
            fuzz1017=None,  # type: ignore[arg-type]
            fuzz1018=None,  # type: ignore[arg-type]
            fuzz1019=None,  # type: ignore[arg-type]
            fuzz1020=None,  # type: ignore[arg-type]
            fuzz1021=None,  # type: ignore[arg-type]
            fuzz1022=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> AffixFuzzer1:
        """Produce an empty AffixFuzzer1, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    fuzz1001: components.AffixFuzzer1Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer1Batch._required,  # type: ignore[misc]
    )
    fuzz1002: components.AffixFuzzer2Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer2Batch._required,  # type: ignore[misc]
    )
    fuzz1003: components.AffixFuzzer3Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer3Batch._required,  # type: ignore[misc]
    )
    fuzz1004: components.AffixFuzzer4Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer4Batch._required,  # type: ignore[misc]
    )
    fuzz1005: components.AffixFuzzer5Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer5Batch._required,  # type: ignore[misc]
    )
    fuzz1006: components.AffixFuzzer6Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer6Batch._required,  # type: ignore[misc]
    )
    fuzz1007: components.AffixFuzzer7Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer7Batch._required,  # type: ignore[misc]
    )
    fuzz1008: components.AffixFuzzer8Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer8Batch._required,  # type: ignore[misc]
    )
    fuzz1009: components.AffixFuzzer9Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer9Batch._required,  # type: ignore[misc]
    )
    fuzz1010: components.AffixFuzzer10Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer10Batch._required,  # type: ignore[misc]
    )
    fuzz1011: components.AffixFuzzer11Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer11Batch._required,  # type: ignore[misc]
    )
    fuzz1012: components.AffixFuzzer12Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer12Batch._required,  # type: ignore[misc]
    )
    fuzz1013: components.AffixFuzzer13Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer13Batch._required,  # type: ignore[misc]
    )
    fuzz1014: components.AffixFuzzer14Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer14Batch._required,  # type: ignore[misc]
    )
    fuzz1015: components.AffixFuzzer15Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer15Batch._required,  # type: ignore[misc]
    )
    fuzz1016: components.AffixFuzzer16Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer16Batch._required,  # type: ignore[misc]
    )
    fuzz1017: components.AffixFuzzer17Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer17Batch._required,  # type: ignore[misc]
    )
    fuzz1018: components.AffixFuzzer18Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer18Batch._required,  # type: ignore[misc]
    )
    fuzz1019: components.AffixFuzzer19Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer19Batch._required,  # type: ignore[misc]
    )
    fuzz1020: components.AffixFuzzer20Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer20Batch._required,  # type: ignore[misc]
    )
    fuzz1021: components.AffixFuzzer21Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer21Batch._required,  # type: ignore[misc]
    )
    fuzz1022: components.AffixFuzzer22Batch = field(
        metadata={"component": "required"},
        converter=components.AffixFuzzer22Batch._required,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
