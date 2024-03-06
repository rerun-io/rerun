from __future__ import annotations

import itertools
from typing import Optional, cast

from rerun.blueprint.archetypes.space_view_contents import SpaceViewContents
from rerun.blueprint.components.entities_determined_by_user import (
    EntitiesDeterminedByUser,
    EntitiesDeterminedByUserBatch,
    EntitiesDeterminedByUserLike,
)
from rerun.blueprint.components.query_expression import QueryExpression, QueryExpressionBatch
from rerun.datatypes.utf8 import Utf8Like

from .common_arrays import none_empty_or_value


def test_space_view_contents() -> None:
    query_array = ["+ /**\n- /robot", QueryExpression("+ /**\n- /robot")]
    entities_determined_by_user_arrays = [False, EntitiesDeterminedByUser(False), None]

    all_arrays = itertools.zip_longest(
        query_array,
        entities_determined_by_user_arrays,
    )

    for query, entities_determined_by_user in all_arrays:
        query = query if query is not None else query_array[-1]

        # mypy can't track types properly through itertools zip so re-cast
        query = cast(Utf8Like, query)
        entities_determined_by_user = cast(Optional[EntitiesDeterminedByUserLike], entities_determined_by_user)

        print(
            "rr.SpaceViewContents(\n",
            f"    {query!r}\n",
            f"    entities_determined_by_user={entities_determined_by_user!r}\n",
            ")",
        )
        arch = SpaceViewContents(
            query,
            entities_determined_by_user=entities_determined_by_user,
        )
        print(f"{arch}\n")

        # Equality checks on some of these are a bit silly, but at least they test out that the serialization code runs without problems.
        assert arch.query == QueryExpressionBatch("+ /**\n- /robot")
        assert arch.entities_determined_by_user == EntitiesDeterminedByUserBatch._optional(
            none_empty_or_value(arch.entities_determined_by_user, False)
        )
