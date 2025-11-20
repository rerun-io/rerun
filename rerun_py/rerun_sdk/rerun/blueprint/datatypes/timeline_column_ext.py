from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from ... import datatypes


class TimelineColumnExt:
    """Extension for [TimelineColumn][rerun.blueprint.datatypes.TimelineColumn]."""

    def __init__(self: Any, timeline: datatypes.Utf8Like, *, visible: datatypes.BoolLike = True) -> None:
        """
        Create a new instance of the TextLogColumn datatype.

        Parameters
        ----------
        timeline:
            What timeline is this?

        visible:
            Is this column visible?

        """

        self.__attrs_init__(visible=visible, timeline=timeline)
