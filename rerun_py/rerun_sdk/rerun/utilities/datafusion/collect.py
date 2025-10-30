from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from datafusion import DataFrame, Expr


def collect_to_string_list(df: DataFrame, col: str | Expr, remove_nulls: bool = True) -> list[str | None]:
    """
    Collect a single column of a DataFrame into a Python string list.

    This is a convenience function. DataFusion collection returns a stream
    of record batches. Sometimes it is preferable to extract a single column
    out of all of these batches and convert it to a string.

    Parameters
    ----------
    df:
        The input DataFusion DataFrame
    col:
        The column to collect. You can provide either a string column
        name or a DataFusion expression.
    remove_nulls:
        If true, any `null` values will be removed from the result. If false
        these will be converted into None.

    """
    batches = df.select(col).collect()

    # Dataframe.collect() will return a list of record batches, but we only care
    # about a single column. We want to combine the results of all batches into
    # a single list. We also know from the above `select` that we will get a
    # single column of data.

    if remove_nulls:
        return [str(r) for rss in batches for rs in rss for r in rs if r.is_valid]
    return [str(r) if r.is_valid else None for rss in batches for rs in rss for r in rs]
