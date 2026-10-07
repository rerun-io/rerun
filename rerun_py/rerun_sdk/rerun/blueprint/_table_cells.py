"""Cell renderers for the unstable table blueprint API."""

from __future__ import annotations

from dataclasses import dataclass
from typing import ClassVar

from .api import View
from .components import TableCellKind

__all__ = [
    "EntryKindCell",
    "FlagCell",
    "LinkCell",
    "PreviewCell",
    "ThumbnailCell",
]


class _Cell:
    """Internal base for concrete table cell renderers."""

    _kind: ClassVar[TableCellKind]

    def __new__(cls, *_args: object, **_kwargs: object) -> _Cell:
        if cls is _Cell:
            raise TypeError("Table cell renderers must use a concrete cell type")
        return super().__new__(cls)


@dataclass(frozen=True)
class LinkCell(_Cell):
    """Render a Rerun URI as an interactive link."""

    _kind: ClassVar[TableCellKind] = TableCellKind.Link


@dataclass(frozen=True)
class ThumbnailCell(_Cell):
    """Render an image blob as a thumbnail."""

    _kind: ClassVar[TableCellKind] = TableCellKind.Thumbnail


@dataclass(frozen=True)
class FlagCell(_Cell):
    """
    Render a boolean as a flag.

    Editable columns currently require this renderer.
    """

    _kind: ClassVar[TableCellKind] = TableCellKind.Flag


@dataclass(frozen=True)
class EntryKindCell(_Cell):
    """Render an integer as a human-readable Rerun entry kind."""

    _kind: ClassVar[TableCellKind] = TableCellKind.EntryKind


@dataclass(frozen=True, init=False)
class PreviewCell(_Cell):
    """
    Render a recording reference using one or more embedded views.

    Preview timing is shared by all preview columns through [`PreviewsConfig`][rerun.blueprint.table.PreviewsConfig].
    """

    _kind: ClassVar[TableCellKind] = TableCellKind.Preview
    views: tuple[View, ...]

    def __init__(self, *views: View) -> None:
        if not views:
            raise ValueError("PreviewCell requires at least one view")
        if not all(isinstance(view, View) for view in views):
            raise TypeError("PreviewCell only accepts Rerun views")
        object.__setattr__(self, "views", tuple(views))


# TODO(RR-4810): Add TimestampCell after TableCellKind models it.
