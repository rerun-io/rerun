from __future__ import annotations

import itertools
from typing import Optional, cast

from rerun.blueprint.archetypes.space_view_blueprint import SpaceViewBlueprint
from rerun.blueprint.components.space_view_class import SpaceViewClass, SpaceViewClassBatch
from rerun.blueprint.components.space_view_origin import SpaceViewOrigin, SpaceViewOriginBatch
from rerun.blueprint.components.visible import Visible, VisibleBatch, VisibleLike
from rerun.components.name import Name, NameBatch
from rerun.datatypes.entity_path import EntityPathLike
from rerun.datatypes.utf8 import Utf8Like

from .common_arrays import none_empty_or_value


def test_space_view_blueprint() -> None:
    class_identifier_arrays = ["3D", SpaceViewClass("3D")]
    display_name_arrays = ["3D View", Name("3D View"), None]
    space_origin_arrays = ["/robot/arm", None, SpaceViewOrigin("/robot/arm")]
    visible_arrays = [False, Visible(False), None]

    all_arrays = itertools.zip_longest(
        class_identifier_arrays,
        display_name_arrays,
        space_origin_arrays,
        visible_arrays,
    )

    for class_identifier, display_name, space_origin, visible in all_arrays:
        class_identifier = class_identifier if class_identifier is not None else class_identifier_arrays[-1]

        # mypy can't track types properly through itertools zip so re-cast
        class_identifier = cast(Utf8Like, class_identifier)
        display_name = cast(Optional[Utf8Like], display_name)
        space_origin = cast(Optional[EntityPathLike], space_origin)
        visible = cast(Optional[VisibleLike], visible)

        print(
            "rr.SpaceViewBlueprint(\n",
            f"    class_identifier={class_identifier!r}\n",
            f"    display_name={display_name!r}\n",
            f"    space_origin={space_origin!r}\n",
            f"    visible={visible!r}\n",
            ")",
        )
        arch = SpaceViewBlueprint(
            class_identifier,
            display_name=display_name,
            space_origin=space_origin,
            visible=visible,
        )
        print(f"{arch}\n")

        # Equality checks on some of these are a bit silly, but at least they test out that the serialization code runs without problems.
        assert arch.class_identifier == SpaceViewClassBatch("3D")
        assert arch.display_name == NameBatch._optional(none_empty_or_value(display_name, "3D View"))
        assert arch.space_origin == SpaceViewOriginBatch._optional(none_empty_or_value(space_origin, "/robot/arm"))
        assert arch.visible == VisibleBatch._optional(none_empty_or_value(visible, False))
