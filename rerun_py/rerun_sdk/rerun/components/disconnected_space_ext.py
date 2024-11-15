from __future__ import annotations

from typing import Any


class DisconnectedSpaceExt:
    """Extension for [DisconnectedSpace][rerun.components.DisconnectedSpace]."""

    def __init__(
        self: Any,
        is_disconnected: bool = True,
    ):
        """
        Disconnect an entity from its parent.

        Parameters
        ----------
        is_disconnected:
            Whether or not the entity should be disconnected from the rest of the scene.
            Set to `True` to disconnect the entity from its parent.
            Set to `False` to disable the effects of this component, (re-)connecting the entity to its parent again.

        """
        self.__attrs_init__(is_disconnected)
