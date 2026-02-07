from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, cast

from rerun.blueprint.archetypes.viewport_blueprint import ViewportBlueprint
from rerun.blueprint.components.auto_layout import AutoLayoutBatch
from rerun.blueprint.components.auto_views import AutoViewsBatch
from rerun.blueprint.components.root_container import RootContainerBatch
from rerun.blueprint.components.view_maximized import ViewMaximizedBatch
from rerun.blueprint.components.viewer_recommendation_hash import (
    ViewerRecommendationHash,
    ViewerRecommendationHashBatch,
)

from .common_arrays import none_empty_or_value, uuid_bytes0, uuid_bytes1

if TYPE_CHECKING:
    from rerun.datatypes.bool import BoolLike
    from rerun.datatypes.uint64 import UInt64ArrayLike
    from rerun.datatypes.uuid import UuidLike


def test_viewport_blueprint() -> None:
    root_container_arrays = [
        None,
        uuid_bytes0,
    ]
    maximized_arrays = [
        None,
        uuid_bytes1,
    ]
    auto_layout_arrays = [None, True]
    auto_views_arrays = [None, False]
    viewer_recommendation_hash_arrays = [
        None,
        [123, 321],
        [ViewerRecommendationHash(123), ViewerRecommendationHash(321)],
    ]

    all_arrays = itertools.zip_longest(
        root_container_arrays,
        maximized_arrays,
        auto_layout_arrays,
        auto_views_arrays,
        viewer_recommendation_hash_arrays,
    )

    for (
        root_container,
        maximized,
        auto_layout,
        auto_views,
        past_viewer_recommendations,
    ) in all_arrays:
        # mypy can't track types properly through itertools zip so re-cast
        root_container = cast("UuidLike | None", root_container)
        maximized = cast("UuidLike | None", maximized)
        auto_layout = cast("BoolLike | None", auto_layout)
        auto_views = cast("BoolLike | None", auto_views)
        past_viewer_recommendations = cast("UInt64ArrayLike | None", past_viewer_recommendations)

        print(
            "rr.ViewportBlueprint(\n",
            f"    root_container={root_container!r}\n",
            f"    maximized={maximized!r}\n",
            f"    auto_layout={auto_layout!r}\n",
            f"    auto_views={auto_views!r}\n",
            f"    past_viewer_recommendations={past_viewer_recommendations!r}\n",
            ")",
        )
        arch = ViewportBlueprint(
            root_container=root_container,
            maximized=maximized,
            auto_layout=auto_layout,
            auto_views=auto_views,
            past_viewer_recommendations=past_viewer_recommendations,
        )
        print(f"{arch}\n")

        assert arch.root_container == RootContainerBatch._converter(none_empty_or_value(root_container, uuid_bytes0))
        assert arch.maximized == ViewMaximizedBatch._converter(none_empty_or_value(maximized, uuid_bytes1))
        assert arch.auto_layout == AutoLayoutBatch._converter(none_empty_or_value(auto_layout, [True]))
        assert arch.auto_views == AutoViewsBatch._converter(none_empty_or_value(auto_views, [False]))
        assert arch.past_viewer_recommendations == ViewerRecommendationHashBatch._converter(
            none_empty_or_value(past_viewer_recommendations, [123, 321]),
        )
