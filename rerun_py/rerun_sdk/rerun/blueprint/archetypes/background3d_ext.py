from __future__ import annotations

from typing import Any

from ... import datatypes
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions


class Background3DExt:
    """Extension for [Background3D][rerun.blueprint.archetypes.Background3D]."""

    def __init__(
        self: Any,
        color: datatypes.Rgba32Like | None = None,
        *,
        kind: blueprint_components.Background3DKindLike | None = None,
    ):
        """
        Create a new instance of the Background3D archetype.

        Parameters
        ----------
        kind:
            The type of the background. Defaults to DirectionalGradient
        color:
            Color used for Background3DKind.SolidColor.

            Defaults to White.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if kind is None:
                if color is None:
                    kind = blueprint_components.Background3DKind.GradientDark
                else:
                    kind = blueprint_components.Background3DKind.SolidColor

            self.__attrs_init__(kind=kind, color=color)
            return
        self.__attrs_clear__()
