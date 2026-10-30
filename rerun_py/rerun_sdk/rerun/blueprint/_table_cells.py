"""Preview columns for the unstable table blueprint API."""

from __future__ import annotations

from dataclasses import dataclass

from .api import View

__all__ = [
    "PreviewCell",
]


@dataclass(frozen=True, init=False)
class PreviewCell:
    """
    Show a recording reference using one or more embedded views.

    Preview timing is shared by all preview columns through [`PreviewsConfig`][rerun.blueprint.table.PreviewsConfig].
    """

    views: tuple[View, ...]

    def __init__(self, *views: View) -> None:
        if not views:
            raise ValueError("PreviewCell requires at least one view")
        if not all(isinstance(view, View) for view in views):
            raise TypeError("PreviewCell only accepts Rerun views")
        object.__setattr__(self, "views", tuple(views))
