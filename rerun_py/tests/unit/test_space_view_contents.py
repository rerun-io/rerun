from __future__ import annotations

import itertools
from typing import cast

from rerun.blueprint.archetypes.space_view_contents import SpaceViewContents
from rerun.blueprint.components.query_expression import QueryExpression, QueryExpressionBatch
from rerun.datatypes.utf8 import Utf8Like


def test_space_view_contents() -> None:
    query_array = ["+ /**\n- /robot", QueryExpression("+ /**\n- /robot")]

    all_arrays = itertools.zip_longest(
        query_array,
    )

    for query, entities_determined_by_user in all_arrays:
        query = query if query is not None else query_array[-1]

        # mypy can't track types properly through itertools zip so re-cast
        query = cast(Utf8Like, query)

        print(
            "rr.SpaceViewContents(\n",
            f"    {query!r}\n",
            ")",
        )
        arch = SpaceViewContents(
            query,
        )
        print(f"{arch}\n")

        # Equality checks on some of these are a bit silly, but at least they test out that the serialization code runs without problems.
        assert arch.query == QueryExpressionBatch("+ /**\n- /robot")
