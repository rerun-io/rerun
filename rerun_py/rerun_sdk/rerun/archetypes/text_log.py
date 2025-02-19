# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/text_log.fbs".

# You can extend this class by creating a "TextLogExt" class in "text_log_ext.py".

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

__all__ = ["TextLog"]


@define(str=False, repr=False, init=False)
class TextLog(Archetype):
    """
    **Archetype**: A log entry in a text log, comprised of a text body and its log level.

    Example
    -------
    ### `text_log_integration`:
    ```python
    import logging

    import rerun as rr

    rr.init("rerun_example_text_log_integration", spawn=True)

    # Log a text entry directly
    rr.log("logs", rr.TextLog("this entry has loglevel TRACE", level=rr.TextLogLevel.TRACE))

    # Or log via a logging handler
    logging.getLogger().addHandler(rr.LoggingHandler("logs/handler"))
    logging.getLogger().setLevel(-1)
    logging.info("This INFO log got added through the standard logging interface")
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/text_log_integration/9737d0c986325802a9885499d6fcc773b1736488/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/text_log_integration/9737d0c986325802a9885499d6fcc773b1736488/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/text_log_integration/9737d0c986325802a9885499d6fcc773b1736488/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/text_log_integration/9737d0c986325802a9885499d6fcc773b1736488/1200w.png">
      <img src="https://static.rerun.io/text_log_integration/9737d0c986325802a9885499d6fcc773b1736488/full.png" width="640">
    </picture>
    </center>

    """

    def __init__(
        self: Any,
        text: datatypes.Utf8Like,
        *,
        level: datatypes.Utf8Like | None = None,
        color: datatypes.Rgba32Like | None = None,
    ):
        """
        Create a new instance of the TextLog archetype.

        Parameters
        ----------
        text:
            The body of the message.
        level:
            The verbosity level of the message.

            This can be used to filter the log messages in the Rerun Viewer.
        color:
            Optional color to use for the log line in the Rerun Viewer.

        """

        # You can define your own __init__ function as a member of TextLogExt in text_log_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(text=text, level=level, color=color)
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            text=None,
            level=None,
            color=None,
        )

    @classmethod
    def _clear(cls) -> TextLog:
        """Produce an empty TextLog, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        text: datatypes.Utf8Like | None = None,
        level: datatypes.Utf8Like | None = None,
        color: datatypes.Rgba32Like | None = None,
    ) -> TextLog:
        """
        Update only some specific fields of a `TextLog`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        text:
            The body of the message.
        level:
            The verbosity level of the message.

            This can be used to filter the log messages in the Rerun Viewer.
        color:
            Optional color to use for the log line in the Rerun Viewer.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "text": text,
                "level": level,
                "color": color,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> TextLog:
        """Clear all the fields of a `TextLog`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        text: datatypes.Utf8ArrayLike | None = None,
        level: datatypes.Utf8ArrayLike | None = None,
        color: datatypes.Rgba32ArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        text:
            The body of the message.
        level:
            The verbosity level of the message.

            This can be used to filter the log messages in the Rerun Viewer.
        color:
            Optional color to use for the log line in the Rerun Viewer.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                text=text,
                level=level,
                color=color,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {"text": text, "level": level, "color": color}
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[arg-type]
                shape = np.shape(param)

                batch_length = shape[1] if len(shape) > 1 else 1
                num_rows = shape[0] if len(shape) >= 1 else 1
                lengths = batch_length * np.ones(num_rows)
            else:
                # For non-primitive types, default to partitioning each element separately.
                lengths = np.ones(len(arrow_array))

            columns.append(batch.partition(lengths))

        indicator_column = cls.indicator().partition(np.zeros(len(lengths)))
        return ComponentColumnList([indicator_column] + columns)

    text: components.TextBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.TextBatch._converter,  # type: ignore[misc]
    )
    # The body of the message.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    level: components.TextLogLevelBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.TextLogLevelBatch._converter,  # type: ignore[misc]
    )
    # The verbosity level of the message.
    #
    # This can be used to filter the log messages in the Rerun Viewer.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    color: components.ColorBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ColorBatch._converter,  # type: ignore[misc]
    )
    # Optional color to use for the log line in the Rerun Viewer.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
