"""
Row filters for the unstable table blueprint API.

Every filter built with `col` writes the same SQL as one of the editable filters in the Viewer's
filter bar, so the Viewer shows it as that filter. This module only has filters the filter bar can
show. Their SQL matches the `to_sql` of the filters in `re_dataframe_ui/src/filters/`.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from datetime import UTC, datetime
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import builtins
    from collections.abc import Callable

__all__ = [
    "Filter",
    "FilterColumn",
    "ListFilters",
    "StringFilters",
    "col",
]

Number = int | float
"""A number a column can be compared to."""

# Name of the lambda parameter for the values of a list column.
_LAMBDA_PARAMETER = "x"

# Escape character in the `LIKE` patterns of string filters.
_LIKE_ESCAPE = "\\"


@dataclass(frozen=True)
class Filter:
    """
    A SQL expression that selects the rows of a table.

    Build filters for one column with [`col`][rerun.blueprint.table.col], which writes the SQL
    the Viewer shows as an editable filter in the filter bar.
    Any other SQL expression also filters the table, and the Viewer shows it as SQL text.

    A filter has no truth value, so `not`, `and` and `or` raise a `TypeError`.

    Parameters
    ----------
    sql:
        A SQL expression that evaluates to a boolean, in the generic SQL dialect of DataFusion.
        Column names are double-quoted, e.g. `"task" ILIKE '%pick%'`.
        Functions that combine rows, such as `sum` or `count`, are not allowed.

    """

    sql: str

    def __post_init__(self) -> None:
        if not isinstance(self.sql, str):
            raise TypeError("Filter.sql must be a string")
        if not self.sql.strip():
            raise ValueError("Filter.sql must not be empty")

    def __bool__(self) -> bool:
        raise TypeError(
            "A filter has no truth value. Use the negated methods of `col`, such as `ne`, "
            "and pass several filters in a list instead of using `and`."
        )

    def __str__(self) -> str:
        return self.sql


def col(name: str) -> FilterColumn:
    """
    Start a filter on the column `name`.

    ```python
    import rerun.blueprint as rrb
    from rerun.blueprint.table import col

    rrb.TableBlueprint(
        filters=[
            col("score") >= 3,
            col("success").is_true(),
            col("task").str.contains("pick"),
            col("created").last_week(),
            col("tags").list.does_not_contain("broken"),
        ],
    )
    ```

    For a list column, filter its values with [`list`][rerun.blueprint.table.FilterColumn.list].

    Parameters
    ----------
    name:
        Physical Arrow column name.

    """
    return FilterColumn(name)


class _ValueFilters:
    """
    The filters of the values of a column, shared by `FilterColumn` and `ListFilters`.

    In this class `str` is the `str` property, so annotations use `builtins.str`.
    """

    __hash__ = None  # type: ignore[assignment]

    def __init__(self, name: builtins.str, *, is_list: bool) -> None:
        self._name = name
        self._is_list = is_list

    def _sql(self) -> builtins.str:
        return _quote_ident(self._name)

    def _predicate(self, predicate: Callable[[builtins.str], builtins.str]) -> Filter:
        """
        Select the rows where the predicate holds.

        `predicate` gets the SQL of one value.
        For a list column, a row is selected when the predicate holds for any of its values.
        """
        if self._is_list:
            return Filter(f"any_match({self._sql()}, {_LAMBDA_PARAMETER} -> {predicate(_LAMBDA_PARAMETER)})")
        return Filter(predicate(self._sql()))

    def _compare(self, operator: builtins.str, value_sql: builtins.str) -> Filter:
        return self._predicate(lambda v: f"{v} {operator} {value_sql}")

    # --- Numbers ---

    def eq(self, value: Number) -> Filter:
        """Select rows where the value equals `value`. Same as `col(…) == value`."""
        return self._compare("=", _number_sql(value))

    def ne(self, value: Number) -> Filter:
        """Select rows where the value is not `value`, or is null. Same as `col(…) != value`."""
        return _negated(self.eq(value))

    def lt(self, value: Number) -> Filter:
        """Select rows where the value is less than `value`. Same as `col(…) < value`."""
        return self._compare("<", _number_sql(value))

    def le(self, value: Number) -> Filter:
        """Select rows where the value is at most `value`. Same as `col(…) <= value`."""
        return self._compare("<=", _number_sql(value))

    def gt(self, value: Number) -> Filter:
        """Select rows where the value is greater than `value`. Same as `col(…) > value`."""
        return self._compare(">", _number_sql(value))

    def ge(self, value: Number) -> Filter:
        """Select rows where the value is at least `value`. Same as `col(…) >= value`."""
        return self._compare(">=", _number_sql(value))

    def __eq__(self, value: Number) -> Filter:  # type: ignore[override]  # ty: ignore[invalid-method-override]
        return self.eq(value)

    def __ne__(self, value: Number) -> Filter:  # type: ignore[override]  # ty: ignore[invalid-method-override]
        return self.ne(value)

    def __lt__(self, value: Number) -> Filter:
        return self.lt(value)

    def __le__(self, value: Number) -> Filter:
        return self.le(value)

    def __gt__(self, value: Number) -> Filter:
        return self.gt(value)

    def __ge__(self, value: Number) -> Filter:
        return self.ge(value)

    # --- Booleans ---

    def is_true(self) -> Filter:
        """Select rows where the value is true."""
        return self._compare("=", "true")

    def is_false(self) -> Filter:
        """Select rows where the value is false."""
        return self._compare("=", "false")

    def is_not_true(self) -> Filter:
        """Select rows where the value is false or null."""
        return _negated(self.is_true())

    def is_not_false(self) -> Filter:
        """Select rows where the value is true or null."""
        return _negated(self.is_false())

    def is_not_null(self) -> Filter:
        """
        Select rows where the value is not null.

        For a list column, this selects non-empty lists without nulls.
        """
        return _negated(self.is_null())

    def is_null(self) -> Filter:
        """
        Select rows where the value is null.

        For a list column, this selects null rows, empty lists, and lists that contain a null.
        """
        sql = self._predicate(lambda v: f"{v} IS NULL").sql
        if self._is_list:
            column = self._sql()
            # `NOT any_match(…, x -> true)` selects empty lists.
            is_empty = f"NOT {self._predicate(lambda _: 'true').sql}"
            sql = f"{column} IS NULL OR {is_empty} OR {sql}"
        return Filter(sql)

    # --- Timestamps ---

    def before(self, timestamp: datetime) -> Filter:
        """
        Select rows with a timestamp before `timestamp`.

        A `datetime` without a time zone is in UTC.
        """
        return self._compare("<", _timestamp_sql(timestamp))

    def after(self, timestamp: datetime) -> Filter:
        """
        Select rows with a timestamp at or after `timestamp`.

        A `datetime` without a time zone is in UTC.
        """
        return self._compare(">=", _timestamp_sql(timestamp))

    def between(self, start: datetime, end: datetime) -> Filter:
        """
        Select rows with a timestamp at or after `start` and before `end`.

        A `datetime` without a time zone is in UTC.
        """
        return self._range(_timestamp_sql(start), _timestamp_sql(end))

    def today(self) -> Filter:
        """Select rows with a timestamp today, in the time zone of the Viewer's query session."""
        return self._range("date_trunc('day', now())", "date_trunc('day', now()) + INTERVAL '1 day'")

    def yesterday(self) -> Filter:
        """Select rows with a timestamp yesterday, in the time zone of the Viewer's query session."""
        return self._range("date_trunc('day', now()) - INTERVAL '1 day'", "date_trunc('day', now())")

    def last_24_hours(self) -> Filter:
        """Select rows with a timestamp in the last 24 hours."""
        return self._range("now() - INTERVAL '24 hours'", "now()")

    def this_week(self) -> Filter:
        """Select rows with a timestamp this week, in the time zone of the Viewer's query session."""
        return self._range("date_trunc('week', now())", "date_trunc('week', now()) + INTERVAL '7 days'")

    def last_week(self) -> Filter:
        """Select rows with a timestamp last week, in the time zone of the Viewer's query session."""
        return self._range("date_trunc('week', now()) - INTERVAL '7 days'", "date_trunc('week', now())")

    def not_between(self, start: datetime, end: datetime) -> Filter:
        """Select rows the [`between`][rerun.blueprint.table.FilterColumn.between] filter does not select."""
        return _negated(self.between(start, end))

    def not_today(self) -> Filter:
        """Select rows the [`today`][rerun.blueprint.table.FilterColumn.today] filter does not select."""
        return _negated(self.today())

    def not_yesterday(self) -> Filter:
        """Select rows the [`yesterday`][rerun.blueprint.table.FilterColumn.yesterday] filter does not select."""
        return _negated(self.yesterday())

    def not_last_24_hours(self) -> Filter:
        """Select rows the [`last_24_hours`][rerun.blueprint.table.FilterColumn.last_24_hours] filter does not select."""
        return _negated(self.last_24_hours())

    def not_this_week(self) -> Filter:
        """Select rows the [`this_week`][rerun.blueprint.table.FilterColumn.this_week] filter does not select."""
        return _negated(self.this_week())

    def not_last_week(self) -> Filter:
        """Select rows the [`last_week`][rerun.blueprint.table.FilterColumn.last_week] filter does not select."""
        return _negated(self.last_week())

    def _range(self, low: builtins.str, high: builtins.str) -> Filter:
        return self._predicate(lambda v: f"{v} >= {low} AND {v} < {high}")

    @property
    def str(self) -> StringFilters:
        """The filters of a string column, e.g. `col("task").str.contains("pick")`."""
        return StringFilters(self)


class FilterColumn(_ValueFilters):
    """
    The column a [`Filter`][rerun.blueprint.table.Filter] applies to.

    Create it with [`col`][rerun.blueprint.table.col].

    Comparisons such as `col("score") >= 3` and the methods of this class return a `Filter`.
    Every filter is one the Viewer shows as an editable filter in its filter bar, when the column has
    the type the filter is for:

    * numbers: [`eq`][rerun.blueprint.table.FilterColumn.eq], [`ne`][rerun.blueprint.table.FilterColumn.ne],
      [`lt`][rerun.blueprint.table.FilterColumn.lt], [`le`][rerun.blueprint.table.FilterColumn.le],
      [`gt`][rerun.blueprint.table.FilterColumn.gt] and [`ge`][rerun.blueprint.table.FilterColumn.ge],
      or the operators `==`, `!=`, `<`, `<=`, `>` and `>=`
    * booleans: [`is_true`][rerun.blueprint.table.FilterColumn.is_true] and
      [`is_false`][rerun.blueprint.table.FilterColumn.is_false], and for nullable columns also
      [`is_null`][rerun.blueprint.table.FilterColumn.is_null],
      [`is_not_true`][rerun.blueprint.table.FilterColumn.is_not_true],
      [`is_not_false`][rerun.blueprint.table.FilterColumn.is_not_false] and
      [`is_not_null`][rerun.blueprint.table.FilterColumn.is_not_null]
    * strings: [`str.contains`][rerun.blueprint.table.StringFilters.contains],
      [`str.does_not_contain`][rerun.blueprint.table.StringFilters.does_not_contain],
      [`str.starts_with`][rerun.blueprint.table.StringFilters.starts_with] and
      [`str.ends_with`][rerun.blueprint.table.StringFilters.ends_with]
    * timestamps: [`before`][rerun.blueprint.table.FilterColumn.before],
      [`after`][rerun.blueprint.table.FilterColumn.after],
      [`between`][rerun.blueprint.table.FilterColumn.between] and the relative ranges such as
      [`today`][rerun.blueprint.table.FilterColumn.today], and the negated ranges such as
      [`not_between`][rerun.blueprint.table.FilterColumn.not_between] and
      [`not_today`][rerun.blueprint.table.FilterColumn.not_today]

    The negated filters select the rows where the filter is false or null.

    `col` doesn't know the type of the column, so nothing stops a filter for another type,
    such as `col("score").str.contains("1")` on a number column, or `col("tags") >= 3` on a list column.
    The Viewer shows such a filter as SQL text.
    If the SQL doesn't work on the column, the Viewer shows the error on that filter and doesn't apply it.

    For a list column, use the same filters on [`list`][rerun.blueprint.table.FilterColumn.list].
    """

    def __init__(self, name: str) -> None:
        if not isinstance(name, str):
            raise TypeError("Filter column name must be a string")
        if not name:
            raise ValueError("Filter column name must not be empty")
        super().__init__(name, is_list=False)

    def __repr__(self) -> str:
        return f"col({self._name!r})"

    @property
    def list(self) -> ListFilters:
        """The values of a list column, e.g. `col("tags").list.str.contains("pick")`."""
        return ListFilters(self._name)


class ListFilters(_ValueFilters):
    """
    The filters of a list column.

    Get them with [`FilterColumn.list`][rerun.blueprint.table.FilterColumn.list],
    e.g. `col("scores").list >= 3` or `col("tags").list.str.contains("pick")`.
    They are the filters of [`FilterColumn`][rerun.blueprint.table.FilterColumn], and a row is
    selected when the filter holds for any value of its list.
    The negated filters, such as `ne`, select the rows where no value matches.
    """

    def __init__(self, name: str) -> None:
        super().__init__(name, is_list=True)

    def __repr__(self) -> str:
        return f"col({self._name!r}).list"


class StringFilters:
    """
    The filters of a string column.

    Get them with [`FilterColumn.str`][rerun.blueprint.table.FilterColumn.str], e.g. `col("task").str.contains("pick")`.
    The text is matched as plain text, ignoring case, not as a regular expression.
    """

    __hash__ = None  # type: ignore[assignment]

    def __init__(self, column: _ValueFilters) -> None:
        self._column = column

    def __repr__(self) -> str:
        return f"{self._column!r}.str"

    def contains(self, text: str) -> Filter:
        """Select rows where the value contains `text`, ignoring case."""
        return self._ilike(text, any_prefix=True, any_suffix=True)

    def does_not_contain(self, text: str) -> Filter:
        """
        Select rows where the value does not contain `text`, ignoring case, or is null.

        For a list column, this selects rows where no value of the list contains `text`.
        """
        return _negated(self.contains(text))

    def starts_with(self, text: str) -> Filter:
        """Select rows where the value starts with `text`, ignoring case."""
        return self._ilike(text, any_prefix=False, any_suffix=True)

    def ends_with(self, text: str) -> Filter:
        """Select rows where the value ends with `text`, ignoring case."""
        return self._ilike(text, any_prefix=True, any_suffix=False)

    def _ilike(self, text: str, *, any_prefix: bool, any_suffix: bool) -> Filter:
        if not isinstance(text, str):
            raise TypeError("String filters require a string")
        if not text:
            raise ValueError("String filters require a non-empty string")

        escaped = "".join(_LIKE_ESCAPE + c if c in "%_\\" else c for c in text)
        pattern = ("%" if any_prefix else "") + escaped + ("%" if any_suffix else "")
        escape_clause = "" if escaped == text else f" ESCAPE {_quote_string(_LIKE_ESCAPE)}"
        return self._column._predicate(lambda v: f"{v} ILIKE {_quote_string(pattern)}{escape_clause}")


def _negated(row_filter: Filter) -> Filter:
    """Select the rows where `row_filter` is false or null, in the form the Viewer shows as a negated filter."""
    return Filter(f"({row_filter.sql}) IS NOT TRUE")


def _number_sql(value: Any) -> str:
    """The SQL literal of a number."""
    # `bool` is a subclass of `int`.
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TypeError(
            f"Can't compare a column to a value of type {type(value).__name__}. "
            "Comparisons take a number. Use `is_true`, `before` or `str.contains` for other columns."
        )
    if isinstance(value, int) or math.isfinite(value):
        return repr(value)
    # SQL has no literals for infinity and NaN.
    return f"CAST({_quote_string('NaN' if math.isnan(value) else repr(value))} AS DOUBLE)"


def _timestamp_sql(value: Any) -> str:
    """The SQL literal of a timestamp."""
    if not isinstance(value, datetime):
        raise TypeError("Timestamp filters require a datetime")
    if value.tzinfo is None:
        value = value.replace(tzinfo=UTC)
    utc = value.astimezone(UTC).replace(tzinfo=None)
    return f"TIMESTAMPTZ {_quote_string(utc.isoformat() + 'Z')}"


def _quote_ident(name: str) -> str:
    return '"' + name.replace('"', '""') + '"'


def _quote_string(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"
