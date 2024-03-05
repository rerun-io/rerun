from __future__ import annotations

import itertools

from rerun.blueprint.archetypes.space_view_blueprint import SpaceViewBlueprint
from rerun.blueprint.components.entities_determined_by_user import (
    EntitiesDeterminedByUser,
    EntitiesDeterminedByUserBatch,
)
from rerun.blueprint.components.included_query import IncludedQuery, IncludedQueryBatch
from rerun.blueprint.components.space_view_class import SpaceViewClass, SpaceViewClassBatch
from rerun.blueprint.components.space_view_origin import SpaceViewOrigin, SpaceViewOriginBatch
from rerun.blueprint.components.visible import Visible, VisibleBatch
from rerun.components.name import Name, NameBatch

from .common_arrays import none_empty_or_value, uuid_bytes0, uuid_bytes1


def test_space_view_blueprint() -> None:
    class_identifier_arrays = ["3D", SpaceViewClass("3D")]
    display_name_arrays = ["3D View", Name("3D View"), None]
    space_origin_arrays = ["/robot/arm", None, SpaceViewOrigin("/robot/arm")]
    entities_determined_by_user_arrays = [False, EntitiesDeterminedByUser(False), None]
    contents_arrays = [[uuid_bytes0, uuid_bytes1], [IncludedQuery(uuid_bytes0), IncludedQuery(uuid_bytes1)], None]
    visible_arrays = [False, Visible(False), None]

    all_arrays = itertools.zip_longest(
        class_identifier_arrays,
        display_name_arrays,
        space_origin_arrays,
        entities_determined_by_user_arrays,
        contents_arrays,
        visible_arrays,
    )

    # for space_views, layout, root_container, maximized, auto_layout, auto_space_views in all_arrays:
    for class_identifier, display_name, space_origin, entities_determined_by_user, contents, visible in all_arrays:
        class_identifier = class_identifier if class_identifier is not None else class_identifier_arrays[-1]

        print(
            "rr.SpaceViewBlueprint(\n",
            f"    class_identifier={class_identifier!r}\n",
            f"    display_name={display_name!r}\n",
            f"    space_origin={space_origin!r}\n",
            f"    entities_determined_by_user={entities_determined_by_user!r}\n",
            f"    contents={contents!r}\n",
            f"    visible={visible!r}\n",
            ")",
        )
        arch = SpaceViewBlueprint(
            class_identifier,
            display_name=display_name,
            space_origin=space_origin,
            entities_determined_by_user=entities_determined_by_user,
            contents=contents,
            visible=visible,
        )
        print(f"{arch}\n")

        # Equality checks on some of these are a bit silly, but at least they test out that the serialization code runs without problems.
        assert arch.class_identifier == SpaceViewClassBatch("3D")
        assert arch.display_name == NameBatch._optional(none_empty_or_value(display_name, "3D View"))
        assert arch.space_origin == SpaceViewOriginBatch._optional(none_empty_or_value(space_origin, "/robot/arm"))
        assert arch.entities_determined_by_user == EntitiesDeterminedByUserBatch._optional(
            none_empty_or_value(arch.entities_determined_by_user, False)
        )
        assert arch.contents == IncludedQueryBatch._optional(none_empty_or_value(contents, [uuid_bytes0, uuid_bytes1]))
        assert arch.visible == VisibleBatch._optional(none_empty_or_value(visible, False))
