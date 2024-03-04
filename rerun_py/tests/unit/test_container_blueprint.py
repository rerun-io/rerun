from __future__ import annotations

import itertools

from rerun.blueprint.archetypes.container_blueprint import ContainerBlueprint
from rerun.blueprint.components.active_tab import ActiveTab, ActiveTabBatch
from rerun.blueprint.components.column_share import ColumnShare, ColumnShareBatch
from rerun.blueprint.components.container_kind import ContainerKind, ContainerKindBatch
from rerun.blueprint.components.grid_columns import GridColumns, GridColumnsBatch
from rerun.blueprint.components.included_content import IncludedContentBatch
from rerun.blueprint.components.row_share import RowShare, RowShareBatch
from rerun.blueprint.components.visible import Visible, VisibleBatch
from rerun.components.name import Name, NameBatch
from rerun.datatypes.entity_path import EntityPath

from .common_arrays import none_empty_or_value


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

    contents_arrays = [
        None,
        [],
        ["space_view/1234", "container/5678"],
        [EntityPath("space_view/1234"), EntityPath("container/5678")],
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
        "space_view/1234",
        ActiveTab("space_view/1234"),
        ActiveTab(EntityPath("space_view/1234")),
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

        assert arch.container_kind == ContainerKindBatch._required(
            none_empty_or_value(container_kind, ContainerKind.Tabs)
        )
        assert arch.display_name == NameBatch._optional(none_empty_or_value(display_name, Name("my container")))
        assert arch.contents == IncludedContentBatch._optional(none_empty_or_value(contents, contents_arrays[-1]))
        assert arch.col_shares == ColumnShareBatch._optional(none_empty_or_value(col_shares, col_shares_arrays[-1]))
        assert arch.row_shares == RowShareBatch._optional(none_empty_or_value(row_shares, row_shares_arrays[-1]))
        assert arch.active_tab == ActiveTabBatch._optional(none_empty_or_value(active_tab, active_tab_arrays[-1]))
        assert arch.visible == VisibleBatch._optional(none_empty_or_value(visible, visible_arrays[-1]))
        assert arch.grid_columns == GridColumnsBatch._optional(
            none_empty_or_value(grid_columns, grid_columns_arrays[-1])
        )
