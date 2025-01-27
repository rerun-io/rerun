# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/testing/archetypes/fuzzy.fbs".

# You can extend this class by creating a "AffixFuzzer3Ext" class in "affix_fuzzer3_ext.py".

from __future__ import annotations

from typing import Any

import numpy as np
from attrs import define, field
from rerun._baseclasses import (
    Archetype,
    ComponentColumnList,
    DescribedComponentBatch,
)
from rerun.error_utils import catch_and_log_exceptions

from .. import components, datatypes

__all__ = ["AffixFuzzer3"]


@define(str=False, repr=False, init=False)
class AffixFuzzer3(Archetype):
    def __init__(
        self: Any,
        *,
        fuzz2001: datatypes.AffixFuzzer1Like | None = None,
        fuzz2002: datatypes.AffixFuzzer1Like | None = None,
        fuzz2003: datatypes.AffixFuzzer1Like | None = None,
        fuzz2004: datatypes.AffixFuzzer1Like | None = None,
        fuzz2005: datatypes.AffixFuzzer1Like | None = None,
        fuzz2006: datatypes.AffixFuzzer1Like | None = None,
        fuzz2007: components.AffixFuzzer7Like | None = None,
        fuzz2008: components.AffixFuzzer8Like | None = None,
        fuzz2009: components.AffixFuzzer9Like | None = None,
        fuzz2010: components.AffixFuzzer10Like | None = None,
        fuzz2011: components.AffixFuzzer11Like | None = None,
        fuzz2012: components.AffixFuzzer12Like | None = None,
        fuzz2013: components.AffixFuzzer13Like | None = None,
        fuzz2014: datatypes.AffixFuzzer3Like | None = None,
        fuzz2015: datatypes.AffixFuzzer3Like | None = None,
        fuzz2016: components.AffixFuzzer16Like | None = None,
        fuzz2017: components.AffixFuzzer17Like | None = None,
        fuzz2018: components.AffixFuzzer18Like | None = None,
    ):
        """Create a new instance of the AffixFuzzer3 archetype."""

        # You can define your own __init__ function as a member of AffixFuzzer3Ext in affix_fuzzer3_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                fuzz2001=fuzz2001,
                fuzz2002=fuzz2002,
                fuzz2003=fuzz2003,
                fuzz2004=fuzz2004,
                fuzz2005=fuzz2005,
                fuzz2006=fuzz2006,
                fuzz2007=fuzz2007,
                fuzz2008=fuzz2008,
                fuzz2009=fuzz2009,
                fuzz2010=fuzz2010,
                fuzz2011=fuzz2011,
                fuzz2012=fuzz2012,
                fuzz2013=fuzz2013,
                fuzz2014=fuzz2014,
                fuzz2015=fuzz2015,
                fuzz2016=fuzz2016,
                fuzz2017=fuzz2017,
                fuzz2018=fuzz2018,
            )
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            fuzz2001=None,
            fuzz2002=None,
            fuzz2003=None,
            fuzz2004=None,
            fuzz2005=None,
            fuzz2006=None,
            fuzz2007=None,
            fuzz2008=None,
            fuzz2009=None,
            fuzz2010=None,
            fuzz2011=None,
            fuzz2012=None,
            fuzz2013=None,
            fuzz2014=None,
            fuzz2015=None,
            fuzz2016=None,
            fuzz2017=None,
            fuzz2018=None,
        )

    @classmethod
    def _clear(cls) -> AffixFuzzer3:
        """Produce an empty AffixFuzzer3, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        fuzz2001: datatypes.AffixFuzzer1Like | None = None,
        fuzz2002: datatypes.AffixFuzzer1Like | None = None,
        fuzz2003: datatypes.AffixFuzzer1Like | None = None,
        fuzz2004: datatypes.AffixFuzzer1Like | None = None,
        fuzz2005: datatypes.AffixFuzzer1Like | None = None,
        fuzz2006: datatypes.AffixFuzzer1Like | None = None,
        fuzz2007: components.AffixFuzzer7Like | None = None,
        fuzz2008: components.AffixFuzzer8Like | None = None,
        fuzz2009: components.AffixFuzzer9Like | None = None,
        fuzz2010: components.AffixFuzzer10Like | None = None,
        fuzz2011: components.AffixFuzzer11Like | None = None,
        fuzz2012: components.AffixFuzzer12Like | None = None,
        fuzz2013: components.AffixFuzzer13Like | None = None,
        fuzz2014: datatypes.AffixFuzzer3Like | None = None,
        fuzz2015: datatypes.AffixFuzzer3Like | None = None,
        fuzz2016: components.AffixFuzzer16Like | None = None,
        fuzz2017: components.AffixFuzzer17Like | None = None,
        fuzz2018: components.AffixFuzzer18Like | None = None,
    ) -> AffixFuzzer3:
        """Update only some specific fields of a `AffixFuzzer3`."""

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "fuzz2001": fuzz2001,
                "fuzz2002": fuzz2002,
                "fuzz2003": fuzz2003,
                "fuzz2004": fuzz2004,
                "fuzz2005": fuzz2005,
                "fuzz2006": fuzz2006,
                "fuzz2007": fuzz2007,
                "fuzz2008": fuzz2008,
                "fuzz2009": fuzz2009,
                "fuzz2010": fuzz2010,
                "fuzz2011": fuzz2011,
                "fuzz2012": fuzz2012,
                "fuzz2013": fuzz2013,
                "fuzz2014": fuzz2014,
                "fuzz2015": fuzz2015,
                "fuzz2016": fuzz2016,
                "fuzz2017": fuzz2017,
                "fuzz2018": fuzz2018,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> AffixFuzzer3:
        """Clear all the fields of a `AffixFuzzer3`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        fuzz2001: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2002: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2003: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2004: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2005: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2006: datatypes.AffixFuzzer1ArrayLike | None = None,
        fuzz2007: components.AffixFuzzer7ArrayLike | None = None,
        fuzz2008: components.AffixFuzzer8ArrayLike | None = None,
        fuzz2009: components.AffixFuzzer9ArrayLike | None = None,
        fuzz2010: components.AffixFuzzer10ArrayLike | None = None,
        fuzz2011: components.AffixFuzzer11ArrayLike | None = None,
        fuzz2012: components.AffixFuzzer12ArrayLike | None = None,
        fuzz2013: components.AffixFuzzer13ArrayLike | None = None,
        fuzz2014: datatypes.AffixFuzzer3ArrayLike | None = None,
        fuzz2015: datatypes.AffixFuzzer3ArrayLike | None = None,
        fuzz2016: components.AffixFuzzer16ArrayLike | None = None,
        fuzz2017: components.AffixFuzzer17ArrayLike | None = None,
        fuzz2018: components.AffixFuzzer18ArrayLike | None = None,
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
                fuzz2001=fuzz2001,
                fuzz2002=fuzz2002,
                fuzz2003=fuzz2003,
                fuzz2004=fuzz2004,
                fuzz2005=fuzz2005,
                fuzz2006=fuzz2006,
                fuzz2007=fuzz2007,
                fuzz2008=fuzz2008,
                fuzz2009=fuzz2009,
                fuzz2010=fuzz2010,
                fuzz2011=fuzz2011,
                fuzz2012=fuzz2012,
                fuzz2013=fuzz2013,
                fuzz2014=fuzz2014,
                fuzz2015=fuzz2015,
                fuzz2016=fuzz2016,
                fuzz2017=fuzz2017,
                fuzz2018=fuzz2018,
            )

        batches = [batch for batch in inst.as_component_batches() if isinstance(batch, DescribedComponentBatch)]
        if len(batches) == 0:
            return ComponentColumnList([])

        lengths = np.ones(len(batches[0]._batch.as_arrow_array()))
        columns = [batch.partition(lengths) for batch in batches]

        indicator_batch = DescribedComponentBatch(cls.indicator(), cls.indicator().component_descriptor())
        indicator_column = indicator_batch.partition(np.zeros(len(lengths)))

        return ComponentColumnList([indicator_column] + columns)

    fuzz2001: components.AffixFuzzer1Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer1Batch._converter,  # type: ignore[misc]
    )
    fuzz2002: components.AffixFuzzer2Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer2Batch._converter,  # type: ignore[misc]
    )
    fuzz2003: components.AffixFuzzer3Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer3Batch._converter,  # type: ignore[misc]
    )
    fuzz2004: components.AffixFuzzer4Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer4Batch._converter,  # type: ignore[misc]
    )
    fuzz2005: components.AffixFuzzer5Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer5Batch._converter,  # type: ignore[misc]
    )
    fuzz2006: components.AffixFuzzer6Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer6Batch._converter,  # type: ignore[misc]
    )
    fuzz2007: components.AffixFuzzer7Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer7Batch._converter,  # type: ignore[misc]
    )
    fuzz2008: components.AffixFuzzer8Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer8Batch._converter,  # type: ignore[misc]
    )
    fuzz2009: components.AffixFuzzer9Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer9Batch._converter,  # type: ignore[misc]
    )
    fuzz2010: components.AffixFuzzer10Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer10Batch._converter,  # type: ignore[misc]
    )
    fuzz2011: components.AffixFuzzer11Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer11Batch._converter,  # type: ignore[misc]
    )
    fuzz2012: components.AffixFuzzer12Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer12Batch._converter,  # type: ignore[misc]
    )
    fuzz2013: components.AffixFuzzer13Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer13Batch._converter,  # type: ignore[misc]
    )
    fuzz2014: components.AffixFuzzer14Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer14Batch._converter,  # type: ignore[misc]
    )
    fuzz2015: components.AffixFuzzer15Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer15Batch._converter,  # type: ignore[misc]
    )
    fuzz2016: components.AffixFuzzer16Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer16Batch._converter,  # type: ignore[misc]
    )
    fuzz2017: components.AffixFuzzer17Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer17Batch._converter,  # type: ignore[misc]
    )
    fuzz2018: components.AffixFuzzer18Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AffixFuzzer18Batch._converter,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
