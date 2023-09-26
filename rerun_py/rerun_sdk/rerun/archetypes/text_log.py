# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/text_log.fbs".

# You can extend this class by creating a "TextLogExt" class in "text_log_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import Archetype

__all__ = ["TextLog"]


@define(str=False, repr=False)
class TextLog(Archetype):
    """A log entry in a text log, comprised of a text body and its log level."""

    def __init__(
        self: Any,
        body: datatypes.Utf8Like,
        level: datatypes.Utf8Like | None = None,
        color: datatypes.ColorLike | None = None,
    ):
        """Create a new instance of the TextLog archetype."""

        # You can define your own __init__ function as a member of TextLogExt in text_log_ext.py
        self.__attrs_init__(body=body, level=level, color=color)

    body: components.TextBatch = field(
        metadata={"component": "required"},
        converter=components.TextBatch,  # type: ignore[misc]
    )
    level: components.TextLogLevelBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TextLogLevelBatch._optional,  # type: ignore[misc]
    )
    color: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
