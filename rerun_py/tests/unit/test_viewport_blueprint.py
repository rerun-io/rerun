from __future__ import annotations

import itertools
from typing import Optional, cast

from rerun.blueprint.archetypes.viewport_blueprint import ViewportBlueprint
from rerun.blueprint.components.auto_layout import AutoLayoutBatch
from rerun.blueprint.components.auto_space_views import AutoSpaceViewsBatch
from rerun.blueprint.components.root_container import RootContainerBatch
from rerun.blueprint.components.space_view_maximized import SpaceViewMaximizedBatch
from rerun.blueprint.components.viewer_recommendation_hash import (
    ViewerRecommendationHash,
    ViewerRecommendationHashBatch,
)
from rerun.datatypes.bool import BoolLike
from rerun.datatypes.uint64 import UInt64ArrayLike
from rerun.datatypes.uuid import UuidLike

from .common_arrays import none_empty_or_value, uuid_bytes0, uuid_bytes1


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
    auto_space_views_arrays = [None, False]
    viewer_recommendation_hash_arrays = [
        None,
        [123, 321],
        [ViewerRecommendationHash(123), ViewerRecommendationHash(321)],
    ]

    all_arrays = itertools.zip_longest(
        root_container_arrays,
        maximized_arrays,
        auto_layout_arrays,
        auto_space_views_arrays,
        viewer_recommendation_hash_arrays,
    )

    for (
        root_container,
        maximized,
        auto_layout,
        auto_space_views,
        past_viewer_recommendations,
    ) in all_arrays:
        # mypy can't track types properly through itertools zip so re-cast
        root_container = cast(Optional[UuidLike], root_container)
        maximized = cast(Optional[UuidLike], maximized)
        auto_layout = cast(Optional[BoolLike], auto_layout)
        auto_space_views = cast(Optional[BoolLike], auto_space_views)
        past_viewer_recommendations = cast(Optional[UInt64ArrayLike], past_viewer_recommendations)

        print(
            "rr.ViewportBlueprint(\n",
            f"    root_container={root_container!r}\n",
            f"    maximized={maximized!r}\n",
            f"    auto_layout={auto_layout!r}\n",
            f"    auto_space_views={auto_space_views!r}\n",
            f"    past_viewer_recommendations={past_viewer_recommendations!r}\n",
            ")",
        )
        arch = ViewportBlueprint(
            root_container=root_container,
            maximized=maximized,
            auto_layout=auto_layout,
            auto_space_views=auto_space_views,
            past_viewer_recommendations=past_viewer_recommendations,
        )
        print(f"{arch}\n")

        assert arch.root_container == RootContainerBatch._optional(none_empty_or_value(root_container, uuid_bytes0))
        assert arch.maximized == SpaceViewMaximizedBatch._optional(none_empty_or_value(maximized, uuid_bytes1))
        assert arch.auto_layout == AutoLayoutBatch._optional(none_empty_or_value(auto_layout, True))
        assert arch.auto_space_views == AutoSpaceViewsBatch._optional(none_empty_or_value(auto_space_views, False))
        assert arch.past_viewer_recommendations == ViewerRecommendationHashBatch._optional(
            none_empty_or_value(past_viewer_recommendations, [123, 321])
        )
