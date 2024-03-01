from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from .clear import Clear


class ClearExt:
    """Extension for [Clear][rerun.archetypes.Clear]."""

    def __init__(
        self: Any,
        *,
        recursive: bool,
    ) -> None:
        """
        Create a new instance of the Clear archetype.

        Parameters
        ----------
        recursive:
             Whether to recursively clear all children.

        """

        # Enforce named parameter and rename parameter to just `recursive`.
        self.__attrs_init__(is_recursive=recursive)

    @staticmethod
    def flat() -> Clear:
        """
        Returns a non-recursive clear archetype.

        This will empty all components of the associated entity at the logged timepoint.
        Children will be left untouched.
        """
        from .clear import Clear

        return Clear(recursive=False)

    @staticmethod
    def recursive() -> Clear:
        """
        Returns a recursive clear archetype.

        This will empty all components of the associated entity at the logged timepoint, as well as
        all components of all its recursive children.
        """
        from .clear import Clear

        return Clear(recursive=True)
