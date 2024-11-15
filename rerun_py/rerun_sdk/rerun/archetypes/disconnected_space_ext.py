from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from ..datatypes import BoolLike


class DisconnectedSpaceExt:
    """Extension for [DisconnectedSpace][rerun.archetypes.DisconnectedSpace]."""

    def __init__(
        self: Any,
        is_disconnected: BoolLike = True,
    ):
        """
        Disconnect an entity from its parent.

        Parameters
        ----------
        is_disconnected:
            Whether or not the entity should be disconnected from the rest of the scene.
            Set to `True` to disconnect the entity from its parent.
            Set to `False` to disable the effects of this archetype, (re-)connecting the entity to its parent again.

        """

        self.__attrs_init__(is_disconnected)
