from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from ... import encodings


class TimelineColumnExt:
    """Extension for [TimelineColumn][rerun.blueprint.encodings.TimelineColumn]."""

    def __init__(self: Any, timeline: encodings.Utf8Like, *, visible: encodings.BoolLike = True) -> None:
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
