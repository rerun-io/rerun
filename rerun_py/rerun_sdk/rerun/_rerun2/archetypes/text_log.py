# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/text_log.fbs".

# You can extend this class by creating a "TextLogExt" class in "text_log_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)

__all__ = ["TextLog"]


@define(str=False, repr=False)
class TextLog(Archetype):
    """A log entry in a text log, comprised of a text body and its log level."""

    # You can define your own __init__ function as a member of TextLogExt in text_log_ext.py

    body: components.TextArray = field(
        metadata={"component": "primary"},
        converter=components.TextArray.from_similar,  # type: ignore[misc]
    )
    level: components.TextLogLevelArray | None = field(
        metadata={"component": "secondary"},
        default=None,
        converter=components.TextLogLevelArray.optional_from_similar,  # type: ignore[misc]
    )
    color: components.ColorArray | None = field(
        metadata={"component": "secondary"},
        default=None,
        converter=components.ColorArray.optional_from_similar,  # type: ignore[misc]
    )
    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
