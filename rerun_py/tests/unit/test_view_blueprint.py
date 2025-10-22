from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, cast

from rerun.blueprint.archetypes.view_blueprint import ViewBlueprint
from rerun.blueprint.components.view_class import ViewClass, ViewClassBatch
from rerun.blueprint.components.view_origin import ViewOrigin, ViewOriginBatch
from rerun.components.name import Name, NameBatch
from rerun.components.visible import Visible, VisibleBatch

from .common_arrays import none_empty_or_value

if TYPE_CHECKING:
    from rerun.datatypes.bool import BoolLike
    from rerun.datatypes.entity_path import EntityPathLike
    from rerun.datatypes.utf8 import Utf8Like


def test_view_blueprint() -> None:
    class_identifier_arrays = ["3D", ViewClass("3D")]
    display_name_arrays = ["3D view", Name("3D view"), None]
    space_origin_arrays = ["/robot/arm", None, ViewOrigin("/robot/arm")]
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
        class_identifier = cast("Utf8Like", class_identifier)
        display_name = cast("Utf8Like | None", display_name)
        space_origin = cast("EntityPathLike | None", space_origin)
        visible = cast("BoolLike | None", visible)

        print(
            "rr.ViewBlueprint(\n",
            f"    class_identifier={class_identifier!r}\n",
            f"    display_name={display_name!r}\n",
            f"    space_origin={space_origin!r}\n",
            f"    visible={visible!r}\n",
            ")",
        )
        arch = ViewBlueprint(
            class_identifier,
            display_name=display_name,
            space_origin=space_origin,
            visible=visible,
        )
        print(f"{arch}\n")

        # Equality checks on some of these are a bit silly, but at least they test out that the serialization code runs without problems.
        assert arch.class_identifier == ViewClassBatch("3D")
        assert arch.display_name == NameBatch._converter(none_empty_or_value(display_name, "3D view"))
        assert arch.space_origin == ViewOriginBatch._converter(none_empty_or_value(space_origin, "/robot/arm"))
        assert arch.visible == VisibleBatch._converter(none_empty_or_value(visible, [False]))
