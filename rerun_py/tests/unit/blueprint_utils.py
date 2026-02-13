from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    import rerun.blueprint as rrb


def assert_blueprint_contents_are_equal(*contents: rrb.View | rrb.Container) -> None:
    """
    Check for equivalence between blueprint contents (views and containers).

    This is done by checking equality between all fields, _except_ the `id` field, which is always unique.
    """

    def strip_id_field(d: dict[str, Any]) -> dict[str, Any]:
        return {k: v for k, v in d.items() if k != "id"}

    for i, (c1, c2) in enumerate(itertools.pairwise(contents)):
        assert strip_id_field(c1.__dict__) == strip_id_field(c2.__dict__), f"View {i} and {i + 1} are not equal"
