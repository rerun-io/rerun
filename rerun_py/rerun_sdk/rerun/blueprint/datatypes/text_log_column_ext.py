from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ... import datatypes

if TYPE_CHECKING:
    from .text_log_column_kind import TextLogColumnKindLike

class TextLogColumnExt:
    """Extension for [TextLogColumn][rerun.blueprint.datatypes.TextLogColumn]."""

    def __init__(self: Any, kind: TextLogColumnKindLike, *, visible: datatypes.BoolLike = True):
        """
        Create a new instance of the TextLogColumn datatype.

        Parameters
        ----------
        kind:
            What kind of column is this?

        visible:
            Is this column visible?

        """

        self.__attrs_init__(visible=visible, kind=kind)
