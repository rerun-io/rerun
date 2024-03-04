from __future__ import annotations

import itertools

from rerun.blueprint.archetypes.viewport_blueprint import ViewportBlueprint
from rerun.blueprint.components.included_space_view import IncludedSpaceViewBatch

from .common_arrays import none_empty_or_value, uuids_arrays

# TODO(andreas): We're obviously nowhere near done with this.


def test_viewport_blueprint() -> None:
    space_views_arrays = uuids_arrays
    # layout_arrays = []
    # root_container_arrays = []
    # maximized_arrays = []
    # auto_layout_arrays = []
    # auto_space_views_arrays = []

    all_arrays = itertools.zip_longest(
        space_views_arrays,
        # layout_arrays,
        # root_container_arrays,
        # maximized_arrays,
        # auto_layout_arrays,
        # auto_space_views_arrays,
    )

    # for space_views, layout, root_container, maximized, auto_layout, auto_space_views in all_arrays:
    for (space_views,) in all_arrays:
        # space_views = space_views if space_views is not None else space_views_arrays[-1]

        print(
            "rr.ViewportBlueprint(\n",
            f"    space_views={space_views!r}\n",
            # f"    layout={layout!r}\n",
            # f"    root_container={root_container!r}\n",
            # f"    maximized={maximized!r}\n",
            # f"    auto_layout={auto_layout!r}\n",
            # f"    auto_space_views={auto_space_views!r}\n",
            ")",
        )
        arch = ViewportBlueprint(
            space_views,
            # layout=layout,
            # root_container=root_container,
            # maximized=maximized,
            # auto_layout=auto_layout,
            # auto_space_views=auto_space_views,
        )
        print(f"{arch}\n")

        assert arch.space_views == IncludedSpaceViewBatch._optional(
            none_empty_or_value(arch.space_views, uuids_arrays[-1])
        )
