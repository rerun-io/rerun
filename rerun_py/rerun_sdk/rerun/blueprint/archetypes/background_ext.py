from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ...error_utils import catch_and_log_exceptions
from .. import components as blueprint_components

if TYPE_CHECKING:
    from ... import datatypes


class BackgroundExt:
    """Extension for [Background][rerun.blueprint.archetypes.Background]."""

    def __init__(
        self: Any,
        color_or_kind: datatypes.Rgba32Like | blueprint_components.BackgroundKindLike | None = None,
        *,
        color: datatypes.Rgba32Like | None = None,
        kind: blueprint_components.BackgroundKindLike | None = None,
    ) -> None:
        """
        Create a new instance of the Background archetype.

        Parameters
        ----------
        color_or_kind:
            Either a color for solid background color or kind of the background (see `BackgroundKind`).
            If set, `color` and `kind` must not be set.

        kind:
            The type of the background. Defaults to BackgroundKind.GradientDark.
        color:
            Color used for BackgroundKind.SolidColor.

            Defaults to White.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if color_or_kind is not None:
                if color is not None or kind is not None:
                    raise ValueError("Only one of `color_or_kind` and `color`/`kind` can be set.")

                if isinstance(color_or_kind, blueprint_components.BackgroundKind):
                    kind = color_or_kind
                else:
                    color = color_or_kind  # type: ignore[assignment]

            if kind is None:
                if color is None:
                    kind = blueprint_components.BackgroundKind.GradientDark
                else:
                    kind = blueprint_components.BackgroundKind.SolidColor

            self.__attrs_init__(kind=kind, color=color)
            return
        self.__attrs_clear__()
