from __future__ import annotations

import itertools

from rerun.blueprint.archetypes.space_view_blueprint import SpaceViewBlueprint
from rerun.blueprint.components.entities_determined_by_user import EntitiesDeterminedByUserBatch
from rerun.blueprint.components.space_view_class import SpaceViewClassBatch
from rerun.blueprint.components.space_view_origin import SpaceViewOriginBatch
from rerun.blueprint.components.visible import VisibleBatch
from rerun.components.name import NameBatch

from .common_arrays import none_empty_or_value


def test_space_view_blueprint() -> None:
    class_identifier_arrays = ["3D", "TimeSeries"]
    display_name_arrays = ["3D View", "Time Series View", None]
    space_origin_arrays = ["/", None, "/robot/arm"]
    entities_determined_by_user_arrays = [True, False, None]
    visible_arrays = [False, True, None]

    all_arrays = itertools.zip_longest(
        class_identifier_arrays,
        display_name_arrays,
        space_origin_arrays,
        entities_determined_by_user_arrays,
        visible_arrays,
    )

    # for space_views, layout, root_container, maximized, auto_layout, auto_space_views in all_arrays:
    for class_identifier, display_name, space_origin, entities_determined_by_user, visible in all_arrays:
        class_identifier = class_identifier if class_identifier is not None else class_identifier_arrays[-1]

        print(
            "rr.SpaceViewBlueprint(\n",
            f"    class_identifier={class_identifier!r}\n",
            f"    display_name={display_name!r}\n",
            f"    space_origin={space_origin!r}\n",
            f"    entities_determined_by_user={entities_determined_by_user!r}\n",
            f"    visible={visible!r}\n",
            ")",
        )
        arch = SpaceViewBlueprint(
            class_identifier,
            display_name=display_name,
            space_origin=space_origin,
            entities_determined_by_user=entities_determined_by_user,
            visible=visible,
        )
        print(f"{arch}\n")

        # Equality checks on some of these are a bit silly, but at least they test out that the serialization code runs without problems.
        assert arch.class_identifier == SpaceViewClassBatch(class_identifier)
        assert arch.display_name == NameBatch._optional(none_empty_or_value(display_name, display_name))
        assert arch.space_origin == SpaceViewOriginBatch._optional(none_empty_or_value(space_origin, space_origin))
        assert arch.entities_determined_by_user == EntitiesDeterminedByUserBatch._optional(
            none_empty_or_value(entities_determined_by_user, entities_determined_by_user)
        )
        assert arch.visible == VisibleBatch._optional(none_empty_or_value(visible, visible))
