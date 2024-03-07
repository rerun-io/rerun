from __future__ import annotations

import itertools
from typing import Optional, cast

from rerun.blueprint.archetypes.viewport_blueprint import ViewportBlueprint
from rerun.blueprint.components.auto_layout import AutoLayoutBatch, AutoLayoutLike
from rerun.blueprint.components.auto_space_views import AutoSpaceViewsBatch, AutoSpaceViewsLike
from rerun.blueprint.components.included_space_view import IncludedSpaceViewBatch
from rerun.blueprint.components.root_container import RootContainerBatch
from rerun.blueprint.components.space_view_maximized import SpaceViewMaximizedBatch
from rerun.blueprint.components.viewer_recommendation_hash import (
    ViewerRecommendationHash,
    ViewerRecommendationHashBatch,
)
from rerun.datatypes.uint64 import UInt64ArrayLike
from rerun.datatypes.uuid import UuidArrayLike, UuidLike

from .common_arrays import none_empty_or_value, uuid_bytes0, uuid_bytes1, uuids_arrays


def test_viewport_blueprint() -> None:
    space_views_arrays = uuids_arrays
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
        space_views_arrays,
        root_container_arrays,
        maximized_arrays,
        auto_layout_arrays,
        auto_space_views_arrays,
        viewer_recommendation_hash_arrays,
    )

    for (
        space_views,
        root_container,
        maximized,
        auto_layout,
        auto_space_views,
        past_viewer_recommendations,
    ) in all_arrays:
        space_views = space_views if space_views is not None else space_views_arrays[-1]

        # mypy can't track types properly through itertools zip so re-cast
        space_views = cast(UuidArrayLike, space_views)
        root_container = cast(Optional[UuidLike], root_container)
        maximized = cast(Optional[UuidLike], maximized)
        auto_layout = cast(Optional[AutoLayoutLike], auto_layout)
        auto_space_views = cast(Optional[AutoSpaceViewsLike], auto_space_views)
        past_viewer_recommendations = cast(Optional[UInt64ArrayLike], past_viewer_recommendations)

        print(
            "rr.ViewportBlueprint(\n",
            f"    space_views={space_views!r}\n",
            f"    root_container={root_container!r}\n",
            f"    maximized={maximized!r}\n",
            f"    auto_layout={auto_layout!r}\n",
            f"    auto_space_views={auto_space_views!r}\n",
            f"    past_viewer_recommendations={past_viewer_recommendations!r}\n",
            ")",
        )
        arch = ViewportBlueprint(
            space_views,
            root_container=root_container,
            maximized=maximized,
            auto_layout=auto_layout,
            auto_space_views=auto_space_views,
            past_viewer_recommendations=past_viewer_recommendations,
        )
        print(f"{arch}\n")

        assert arch.space_views == IncludedSpaceViewBatch._optional(none_empty_or_value(space_views, uuids_arrays[-1]))
        assert arch.root_container == RootContainerBatch._optional(none_empty_or_value(root_container, uuid_bytes0))
        assert arch.maximized == SpaceViewMaximizedBatch._optional(none_empty_or_value(maximized, uuid_bytes1))
        assert arch.auto_layout == AutoLayoutBatch._optional(none_empty_or_value(auto_layout, True))
        assert arch.auto_space_views == AutoSpaceViewsBatch._optional(none_empty_or_value(auto_space_views, False))
        assert arch.past_viewer_recommendations == ViewerRecommendationHashBatch._optional(
            none_empty_or_value(past_viewer_recommendations, [123, 321])
        )
