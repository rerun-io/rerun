# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/recording_properties.fbs".

# You can extend this class by creating a "RecordingPropertiesExt" class in "recording_properties_ext.py".

from __future__ import annotations

from typing import Any

import numpy as np
import pyarrow as pa
from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
    ComponentColumnList,
)
from ..error_utils import catch_and_log_exceptions

__all__ = ["RecordingProperties"]


@define(str=False, repr=False, init=False)
class RecordingProperties(Archetype):
    """**Archetype**: A list of properties associated with a recording."""

    def __init__(
        self: Any, *, started: datatypes.TimeIntLike | None = None, name: datatypes.Utf8Like | None = None
    ) -> None:
        """
        Create a new instance of the RecordingProperties archetype.

        Parameters
        ----------
        started:
            When the recording started.

            Should be an absolute time, i.e. relative to Unix Epoch.
        name:
            A user-chosen name for the recording.

        """

        # You can define your own __init__ function as a member of RecordingPropertiesExt in recording_properties_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(started=started, name=name)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            started=None,
            name=None,
        )

    @classmethod
    def _clear(cls) -> RecordingProperties:
        """Produce an empty RecordingProperties, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        started: datatypes.TimeIntLike | None = None,
        name: datatypes.Utf8Like | None = None,
    ) -> RecordingProperties:
        """
        Update only some specific fields of a `RecordingProperties`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        started:
            When the recording started.

            Should be an absolute time, i.e. relative to Unix Epoch.
        name:
            A user-chosen name for the recording.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "started": started,
                "name": name,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> RecordingProperties:
        """Clear all the fields of a `RecordingProperties`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        started: datatypes.TimeIntArrayLike | None = None,
        name: datatypes.Utf8ArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        started:
            When the recording started.

            Should be an absolute time, i.e. relative to Unix Epoch.
        name:
            A user-chosen name for the recording.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                started=started,
                name=name,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {"started": started, "name": name}
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

    started: components.RecordingStartedTimestampBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.RecordingStartedTimestampBatch._converter,  # type: ignore[misc]
    )
    # When the recording started.
    #
    # Should be an absolute time, i.e. relative to Unix Epoch.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    name: components.RecordingNameBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.RecordingNameBatch._converter,  # type: ignore[misc]
    )
    # A user-chosen name for the recording.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
