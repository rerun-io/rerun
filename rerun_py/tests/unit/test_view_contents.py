from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, cast

from rerun.blueprint.archetypes.view_contents import ViewContents
from rerun.blueprint.components.query_expression import QueryExpression, QueryExpressionBatch

if TYPE_CHECKING:
    from rerun.datatypes.utf8 import Utf8ArrayLike


def test_view_contents() -> None:
    query_array = [
        [
            "+ /**",
            "- /robot",
        ],
        [
            QueryExpression("+ /**"),
            QueryExpression("- /robot"),
        ],
    ]

    all_arrays = itertools.zip_longest(
        query_array,
    )

    for (query,) in all_arrays:
        # query = query if query is not None else query_array[-1]

        # mypy can't track types properly through itertools zip so re-cast
        query = cast("Utf8ArrayLike", query)

        print(
            "rr.ViewContents(\n",
            f"    {query!r}\n",
            ")",
        )
        arch = ViewContents(
            query,
        )

        # Equality checks on some of these are a bit silly, but at least they test out that the serialization code runs without problems.
        assert arch.query == QueryExpressionBatch(["+ /**", "- /robot"])
