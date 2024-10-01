from __future__ import annotations

from typing import Any

from ... import datatypes
from ...blueprint import components as blueprint_components, datatypes as blueprint_datatypes
from ...error_utils import catch_and_log_exceptions


class DataframeQueryV2Ext:
    """Extension for [DataframeQueryV2][rerun.blueprint.archetypes.DataframeQueryV2]."""

    def __init__(
        self: Any,
        *,
        timeline: datatypes.Utf8Like | None = None,
        filter_by_range: tuple[datatypes.TimeInt, datatypes.TimeInt]
        | blueprint_datatypes.RangeFilterLike
        | None = None,
        filter_by_event: blueprint_datatypes.ComponentColumnSelectorLike | None = None,
        apply_latest_at: bool = False,
        selected_columns: list[blueprint_datatypes.ComponentColumnSelectorLike | datatypes.Utf8Like | str]
        | None = None,
    ):
        """
        Create a new instance of the DataframeQueryV2 archetype.

        Parameters
        ----------
        timeline:
            The timeline for this query.

        filter_by_range:
            If set, a range filter is applied.

        filter_by_event:
            If provided, the dataframe will only contain rows corresponding to timestamps at which an event was logged
            for the provided column.

        apply_latest_at:
            Should empty cells be filled with latest-at queries?

        selected_columns:
            Selected columns. If unset, all columns are selected.

        """

        if isinstance(filter_by_range, tuple):
            start, end = filter_by_range
            filter_by_range = blueprint_components.RangeFilter(start, end)

        if filter_by_event is None:
            filter_by_event_active = None
            filter_by_event_column = None
        else:
            filter_by_event_active = True
            if isinstance(filter_by_event, str):
                column = blueprint_datatypes.ComponentColumnSelector(spec=filter_by_event)
            else:
                column = filter_by_event
            filter_by_event_column = column

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                timeline=timeline,
                range_filter=filter_by_range,
                filter_by_event_active=filter_by_event_active,
                filter_by_event_column=filter_by_event_column,
                apply_latest_at=apply_latest_at,
                selected_columns=selected_columns,
            )
            return
        self.__attrs_clear__()
