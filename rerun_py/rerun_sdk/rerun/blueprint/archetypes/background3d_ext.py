from __future__ import annotations

from typing import Any

from ... import datatypes
from ...blueprint import components as blueprint_components
from ...error_utils import catch_and_log_exceptions


class Background3DExt:
    """Extension for [Background3D][rerun.blueprint.archetypes.Background3D]."""

    def __init__(
        self: Any,
        color_or_kind: datatypes.Rgba32Like | blueprint_components.Background3DKindLike | None = None,
        *,
        color: datatypes.Rgba32Like | None = None,
        kind: blueprint_components.Background3DKindLike | None = None,
    ):
        """
        Create a new instance of the Background3D archetype.

        Parameters
        ----------
        color_or_kind:
            Either a color for solid background color or kind of the background (see `Background3DKind`).
            If set, `color` and `kind` must not be set.

        kind:
            The type of the background. Defaults to Background3DKind.GradientDark.
        color:
            Color used for Background3DKind.SolidColor.

            Defaults to White.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if color_or_kind is not None:
                if color is not None or kind is not None:
                    raise ValueError("Only one of `color_or_kind` and `color`/`kind` can be set.")

                if isinstance(color_or_kind, blueprint_components.Background3DKind):
                    kind = color_or_kind
                else:
                    color = color_or_kind  # type: ignore[assignment]

            if kind is None:
                if color is None:
                    kind = blueprint_components.Background3DKind.GradientDark
                else:
                    kind = blueprint_components.Background3DKind.SolidColor

            self.__attrs_init__(kind=kind, color=color)
            return
        self.__attrs_clear__()
