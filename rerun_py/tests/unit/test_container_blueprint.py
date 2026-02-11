from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, Any, cast

from rerun.blueprint.archetypes.container_blueprint import ContainerBlueprint
from rerun.blueprint.components.active_tab import ActiveTab, ActiveTabBatch
from rerun.blueprint.components.column_share import ColumnShare, ColumnShareBatch
from rerun.blueprint.components.container_kind import ContainerKind, ContainerKindBatch, ContainerKindLike
from rerun.blueprint.components.grid_columns import GridColumns, GridColumnsBatch
from rerun.blueprint.components.included_content import IncludedContentBatch
from rerun.blueprint.components.row_share import RowShare, RowShareBatch
from rerun.components.name import Name, NameBatch
from rerun.components.visible import Visible, VisibleBatch
from rerun.datatypes.entity_path import EntityPath, EntityPathArrayLike, EntityPathLike

from .common_arrays import none_empty_or_value

if TYPE_CHECKING:
    from collections.abc import Sequence

    from rerun.datatypes.bool import BoolLike
    from rerun.datatypes.float32 import Float32ArrayLike
    from rerun.datatypes.uint32 import UInt32Like
    from rerun.datatypes.utf8 import Utf8Like


def test_container_blueprint() -> None:
    container_kind_arrays = [
        ContainerKind.Tabs,
        "Tabs",
    ]

    display_name_arrays = [
        None,
        Name("my container"),
        "my container",
    ]

    contents_arrays: Sequence[Any] = [
        None,
        [],
        ["view/1234", "container/5678"],
        [EntityPath("view/1234"), EntityPath("container/5678")],
    ]

    col_shares_arrays = [
        None,
        [0.3, 0.7],
        [ColumnShare(0.3), ColumnShare(0.7)],
    ]

    row_shares_arrays = [
        None,
        [0.4, 0.6],
        [RowShare(0.4), RowShare(0.6)],
    ]

    active_tab_arrays = [
        None,
        "view/1234",
        ActiveTab("view/1234"),
        ActiveTab(EntityPath("view/1234")),
    ]

    visible_arrays = [
        None,
        True,
        Visible(True),
    ]

    grid_columns_arrays = [
        None,
        4,
        GridColumns(4),
    ]

    all_arrays = itertools.zip_longest(
        container_kind_arrays,
        display_name_arrays,
        contents_arrays,
        col_shares_arrays,
        row_shares_arrays,
        active_tab_arrays,
        visible_arrays,
        grid_columns_arrays,
    )

    for (
        container_kind,
        display_name,
        contents,
        col_shares,
        row_shares,
        active_tab,
        visible,
        grid_columns,
    ) in all_arrays:
        container_kind = container_kind if container_kind is not None else container_kind_arrays[-1]

        container_kind = cast("ContainerKindLike", container_kind)
        display_name = cast("Utf8Like | None", display_name)
        contents = cast("EntityPathArrayLike | None", contents)
        col_shares = cast("Float32ArrayLike | None", col_shares)
        row_shares = cast("Float32ArrayLike | None", row_shares)
        active_tab = cast("EntityPathLike | None", active_tab)
        visible = cast("BoolLike | None", visible)
        grid_columns = cast("UInt32Like | None", grid_columns)

        print(
            "rr.ContainerBlueprint(\n",
            f"    container_kind={container_kind!r}\n",
            f"    display_name={display_name!r}\n",
            f"    contents={contents!r}\n",
            f"    col_shares={col_shares!r}\n",
            f"    row_shares={row_shares!r}\n",
            f"    active_tab={active_tab!r}\n",
            f"    visible={visible!r}\n",
            f"    grid_columns={grid_columns!r}\n",
            ")",
        )
        arch = ContainerBlueprint(
            container_kind,
            display_name=display_name,
            contents=contents,
            col_shares=col_shares,
            row_shares=row_shares,
            active_tab=active_tab,
            visible=visible,
            grid_columns=grid_columns,
        )
        print(f"{arch}\n")

        assert arch.container_kind == ContainerKindBatch._converter(
            none_empty_or_value(container_kind, ContainerKind.Tabs),
        )
        assert arch.display_name == NameBatch._converter(none_empty_or_value(display_name, Name("my container")))
        assert arch.contents == IncludedContentBatch._converter(none_empty_or_value(contents, contents_arrays[-1]))
        assert arch.col_shares == ColumnShareBatch._converter(none_empty_or_value(col_shares, col_shares_arrays[-1]))
        assert arch.row_shares == RowShareBatch._converter(none_empty_or_value(row_shares, row_shares_arrays[-1]))
        assert arch.active_tab == ActiveTabBatch._converter(none_empty_or_value(active_tab, active_tab_arrays[-1]))
        assert arch.visible == VisibleBatch._converter(none_empty_or_value(visible, visible_arrays[-1]))  # type: ignore[arg-type]
        assert arch.grid_columns == GridColumnsBatch._converter(
            none_empty_or_value(grid_columns, grid_columns_arrays[-1]),
        )
