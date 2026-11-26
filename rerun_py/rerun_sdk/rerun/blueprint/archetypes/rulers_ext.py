from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ...error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from ... import encodings


class RulersExt:
    """Extension for [Rulers][rerun.blueprint.archetypes.Rulers]."""

    def __init__(
        self: Any,
        visible: encodings.BoolLike | None = None,
        *,
        name_x: encodings.Utf8Like | None = None,
        name_y: encodings.Utf8Like | None = None,
    ) -> None:
        """
        Create a new instance of the Rulers archetype.

        Parameters
        ----------
        visible:
            Whether the rulers are visible.

            Defaults to false.
        name_x:
            Name shown at the end of the horizontal ruler where X is highest.

            Empty by default. An empty name is not shown.
        name_y:
            Name shown at the end of the vertical ruler where Y is highest.

            Empty by default. An empty name is not shown.

        """

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(visible=visible, name_x=name_x, name_y=name_y)
            return
        self.__attrs_clear__()
