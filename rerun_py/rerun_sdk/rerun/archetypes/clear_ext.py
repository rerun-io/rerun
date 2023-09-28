from __future__ import annotations

from typing import Any


class ClearExt:
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

        # Enforce named parameter.
        self.__attrs_init__(recursive=recursive)

    @staticmethod
    def deferred_patch_class(cls: Any) -> None:
        def flat() -> cls:
            return cls(recursive=False)

        def recursive() -> cls:
            return cls(recursive=True)

        cls.flat = flat
        cls.recursive = recursive
