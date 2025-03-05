from __future__ import annotations

from typing import Any


class ClearIsRecursiveExt:
    """Extension for [ClearIsRecursive][rerun.components.ClearIsRecursive]."""

    def __init__(
        self: Any,
        recursive: bool = True,
    ) -> None:
        """
        Disconnect an entity from its parent.

        Parameters
        ----------
        recursive:
            If true, also clears all recursive children entities.

        """
        self.__attrs_init__(recursive)
