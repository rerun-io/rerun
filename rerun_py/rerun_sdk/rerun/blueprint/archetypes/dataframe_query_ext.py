from __future__ import annotations

from typing import Any

from ... import datatypes
from ...blueprint import components as blueprint_components, datatypes as blueprint_datatypes
from ...error_utils import catch_and_log_exceptions


class DataframeQueryExt:
    """Extension for [DataframeQuery][rerun.blueprint.archetypes.DataframeQuery]."""

    def __init__(
        self: Any,
        *,
        timeline: datatypes.Utf8Like | None = None,
        filter_by_range: tuple[datatypes.TimeInt, datatypes.TimeInt]
        | blueprint_datatypes.FilterByRangeLike
        | None = None,
        filter_is_not_null: blueprint_datatypes.ComponentColumnSelectorLike | None = None,
        apply_latest_at: bool = False,
        select: list[blueprint_datatypes.ComponentColumnSelectorLike | datatypes.Utf8Like | str] | None = None,
    ) -> None:
        """
        Create a new instance of the DataframeQuery archetype.

        Parameters
        ----------
        timeline:
            The timeline for this query.

        filter_by_range:
            If set, a range filter is applied.

        filter_is_not_null:
            If provided, the dataframe will only contain rows corresponding to timestamps at which an event was logged
            for the provided column.

        apply_latest_at:
            Should empty cells be filled with latest-at queries?

        select:
            Selected columns. If unset, all columns are selected.

        """

        if isinstance(filter_by_range, tuple):
            start, end = filter_by_range
            filter_by_range = blueprint_components.FilterByRange(start, end)

        if filter_is_not_null is not None:
            if isinstance(filter_is_not_null, str):
                column = blueprint_datatypes.ComponentColumnSelector(spec=filter_is_not_null)
            else:
                column = filter_is_not_null

            new_filter_is_not_null = blueprint_components.FilterIsNotNull(active=True, column=column)
        else:
            new_filter_is_not_null = None

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                timeline=timeline,
                filter_by_range=filter_by_range,
                filter_is_not_null=new_filter_is_not_null,
                apply_latest_at=apply_latest_at,
                select=select,
            )
            return
        self.__attrs_clear__()
