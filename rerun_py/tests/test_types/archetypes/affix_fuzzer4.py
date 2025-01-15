# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer4Ext" class in "affix_fuzzer4_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field
from rerun._baseclasses import (
    Archetype,
)
from rerun.error_utils import catch_and_log_exceptions

from .. import components, datatypes

__all__ = ["AffixFuzzer4"]


@define(str=False, repr=False, init=False)
class AffixFuzzer4(Archetype):
    def __init__(
        self: Any,
        *,
        fuzz2101: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2102: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2103: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2104: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2105: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2106: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2107: components.AffixFuzzer7ArrayLike | None = None,
        fuzz2108: components.AffixFuzzer8ArrayLike | None = None,
        fuzz2109: components.AffixFuzzer9ArrayLike | None = None,
        fuzz2110: components.AffixFuzzer10ArrayLike | None = None,
        fuzz2111: components.AffixFuzzer11ArrayLike | None = None,
        fuzz2112: components.AffixFuzzer12ArrayLike | None = None,
        fuzz2113: components.AffixFuzzer13ArrayLike | None = None,
        fuzz2114: datatypes.AffixFuzzer3ArrayLike | None = None,
        fuzz2115: datatypes.AffixFuzzer3ArrayLike | None = None,
        fuzz2116: components.AffixFuzzer16ArrayLike | None = None,
        fuzz2117: components.AffixFuzzer17ArrayLike | None = None,
        fuzz2118: components.AffixFuzzer18ArrayLike | None = None,
    ):
        """Create a new instance of the AffixFuzzer4 archetype."""

        # You can define your own __init__ function as a member of AffixFuzzer4Ext in affix_fuzzer4_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                fuzz2101=fuzz2101,
                fuzz2102=fuzz2102,
                fuzz2103=fuzz2103,
                fuzz2104=fuzz2104,
                fuzz2105=fuzz2105,
                fuzz2106=fuzz2106,
                fuzz2107=fuzz2107,
                fuzz2108=fuzz2108,
                fuzz2109=fuzz2109,
                fuzz2110=fuzz2110,
                fuzz2111=fuzz2111,
                fuzz2112=fuzz2112,
                fuzz2113=fuzz2113,
                fuzz2114=fuzz2114,
                fuzz2115=fuzz2115,
                fuzz2116=fuzz2116,
                fuzz2117=fuzz2117,
                fuzz2118=fuzz2118,
            )
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            fuzz2101=None,
            fuzz2102=None,
            fuzz2103=None,
            fuzz2104=None,
            fuzz2105=None,
            fuzz2106=None,
            fuzz2107=None,
            fuzz2108=None,
            fuzz2109=None,
            fuzz2110=None,
            fuzz2111=None,
            fuzz2112=None,
            fuzz2113=None,
            fuzz2114=None,
            fuzz2115=None,
            fuzz2116=None,
            fuzz2117=None,
            fuzz2118=None,
        )

    @classmethod
    def _clear(cls) -> AffixFuzzer4:
        """Produce an empty AffixFuzzer4, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        fuzz2101: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2102: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2103: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2104: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2105: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2106: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2107: components.AffixFuzzer7ArrayLike | None = None,
        fuzz2108: components.AffixFuzzer8ArrayLike | None = None,
        fuzz2109: components.AffixFuzzer9ArrayLike | None = None,
        fuzz2110: components.AffixFuzzer10ArrayLike | None = None,
        fuzz2111: components.AffixFuzzer11ArrayLike | None = None,
        fuzz2112: components.AffixFuzzer12ArrayLike | None = None,
        fuzz2113: components.AffixFuzzer13ArrayLike | None = None,
        fuzz2114: datatypes.AffixFuzzer3ArrayLike | None = None,
        fuzz2115: datatypes.AffixFuzzer3ArrayLike | None = None,
        fuzz2116: components.AffixFuzzer16ArrayLike | None = None,
        fuzz2117: components.AffixFuzzer17ArrayLike | None = None,
        fuzz2118: components.AffixFuzzer18ArrayLike | None = None,
    ) -> AffixFuzzer4:
        """Update only some specific fields of a `AffixFuzzer4`."""

        kwargs = {
            "fuzz2101": fuzz2101,
            "fuzz2102": fuzz2102,
            "fuzz2103": fuzz2103,
            "fuzz2104": fuzz2104,
            "fuzz2105": fuzz2105,
            "fuzz2106": fuzz2106,
            "fuzz2107": fuzz2107,
            "fuzz2108": fuzz2108,
            "fuzz2109": fuzz2109,
            "fuzz2110": fuzz2110,
            "fuzz2111": fuzz2111,
            "fuzz2112": fuzz2112,
            "fuzz2113": fuzz2113,
            "fuzz2114": fuzz2114,
            "fuzz2115": fuzz2115,
            "fuzz2116": fuzz2116,
            "fuzz2117": fuzz2117,
            "fuzz2118": fuzz2118,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return AffixFuzzer4(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> AffixFuzzer4:
        """Clear all the fields of a `AffixFuzzer4`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            fuzz2101=[],
            fuzz2102=[],
            fuzz2103=[],
            fuzz2104=[],
            fuzz2105=[],
            fuzz2106=[],
            fuzz2107=[],
            fuzz2108=[],
            fuzz2109=[],
            fuzz2110=[],
            fuzz2111=[],
            fuzz2112=[],
            fuzz2113=[],
            fuzz2114=[],
            fuzz2115=[],
            fuzz2116=[],
            fuzz2117=[],
            fuzz2118=[],
        )
        return inst

    fuzz2101: components.AffixFuzzer1Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer1Batch._optional,  # type: ignore[misc]
    )
    fuzz2102: components.AffixFuzzer2Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer2Batch._optional,  # type: ignore[misc]
    )
    fuzz2103: components.AffixFuzzer3Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer3Batch._optional,  # type: ignore[misc]
    )
    fuzz2104: components.AffixFuzzer4Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer4Batch._optional,  # type: ignore[misc]
    )
    fuzz2105: components.AffixFuzzer5Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer5Batch._optional,  # type: ignore[misc]
    )
    fuzz2106: components.AffixFuzzer6Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer6Batch._optional,  # type: ignore[misc]
    )
    fuzz2107: components.AffixFuzzer7Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer7Batch._optional,  # type: ignore[misc]
    )
    fuzz2108: components.AffixFuzzer8Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer8Batch._optional,  # type: ignore[misc]
    )
    fuzz2109: components.AffixFuzzer9Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer9Batch._optional,  # type: ignore[misc]
    )
    fuzz2110: components.AffixFuzzer10Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer10Batch._optional,  # type: ignore[misc]
    )
    fuzz2111: components.AffixFuzzer11Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer11Batch._optional,  # type: ignore[misc]
    )
    fuzz2112: components.AffixFuzzer12Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer12Batch._optional,  # type: ignore[misc]
    )
    fuzz2113: components.AffixFuzzer13Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer13Batch._optional,  # type: ignore[misc]
    )
    fuzz2114: components.AffixFuzzer14Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer14Batch._optional,  # type: ignore[misc]
    )
    fuzz2115: components.AffixFuzzer15Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer15Batch._optional,  # type: ignore[misc]
    )
    fuzz2116: components.AffixFuzzer16Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer16Batch._optional,  # type: ignore[misc]
    )
    fuzz2117: components.AffixFuzzer17Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer17Batch._optional,  # type: ignore[misc]
    )
    fuzz2118: components.AffixFuzzer18Batch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AffixFuzzer18Batch._optional,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
