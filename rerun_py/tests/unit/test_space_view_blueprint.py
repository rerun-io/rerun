from __future__ import annotations

import itertools

from rerun.blueprint.archetypes.space_view_blueprint import SpaceViewBlueprint
from rerun.blueprint.components.space_view_class import SpaceViewClassBatch
from rerun.components.name import NameBatch

from .common_arrays import none_empty_or_value


def test_space_view_blueprint() -> None:
    class_identifier_arrays = ["3D", "TimeSeries"]
    display_name_arrays = ["3D View", "Time Series View", None]

    all_arrays = itertools.zip_longest(
        class_identifier_arrays,
        display_name_arrays,
    )

    # for space_views, layout, root_container, maximized, auto_layout, auto_space_views in all_arrays:
    for class_identifier, display_name in all_arrays:
        class_identifier = class_identifier if class_identifier is not None else class_identifier_arrays[-1]

        print(
            "rr.SpaceViewBlueprint(\n",
            f"    class_identifier={class_identifier!r}\n",
            f"    display_name={display_name!r}\n",
            ")",
        )
        arch = SpaceViewBlueprint(
            class_identifier,
            display_name=display_name,
        )
        print(f"{arch}\n")

        assert arch.class_identifier == SpaceViewClassBatch(class_identifier)
        assert arch.display_name == NameBatch._optional(none_empty_or_value(display_name, display_name))
