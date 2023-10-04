# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/text_log.fbs".

# You can extend this class by creating a "TextLogExt" class in "text_log_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import Archetype
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
            text=None,  # type: ignore[arg-type]
            level=None,  # type: ignore[arg-type]
            color=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> TextLog:
        """Produce an empty TextLog, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    text: components.TextBatch = field(
        metadata={"component": "required"},
        converter=components.TextBatch._required,  # type: ignore[misc]
    )
    # The body of the message.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    level: components.TextLogLevelBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TextLogLevelBatch._optional,  # type: ignore[misc]
    )
    # The verbosity level of the message.
    #
    # This can be used to filter the log messages in the Rerun Viewer.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    color: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # Optional color to use for the log line in the Rerun Viewer.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
