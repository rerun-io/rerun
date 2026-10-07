from __future__ import annotations

from typing import TYPE_CHECKING, Literal
from unittest.mock import Mock, patch

import pytest
import rerun as rr
import rerun.blueprint as rrb
from rerun.chunk import RrdReader

if TYPE_CHECKING:
    from pathlib import Path


def _save_and_read(blueprint: rrb.TableBlueprint, path: Path) -> dict[str, dict[str, list[object]]]:
    blueprint.save("test_table_blueprint", path)
    reader = RrdReader(path)
    [store] = reader.blueprints()
    result: dict[str, dict[str, list[object]]] = {}
    for chunk in reader.stream(store=store):
        batch = chunk.to_record_batch()
        assert batch.column("blueprint").to_pylist() == [0] * batch.num_rows
        components = result.setdefault(chunk.entity_path, {})
        for field, column in zip(batch.schema, batch.columns, strict=True):
            if (field.metadata or {}).get(rr.RERUN_KIND) == b"data":
                # Null rows are absent component updates, not logged empty batches.
                components.setdefault(field.name, []).extend(row for row in column.to_pylist() if row is not None)
    return result


def test_table_blueprint_logs_layouts_columns_and_preview_views(tmp_path: Path) -> None:
    view = rrb.Spatial2DView(origin="/camera")
    preview = rrb.table.Column("recording/link", name="Recording", cell=rrb.table.PreviewCell(view))
    flag = rrb.table.Column("flag", editable=True, visible=False, cell=rrb.table.FlagCell())
    blueprint = rrb.TableBlueprint(
        table_layout=rrb.table.TableLayout(columns=[preview, flag]),
        card_layout=rrb.table.CardLayout(title="recording/link", link="recording/link", fields=[flag, preview]),
    )
    data = _save_and_read(blueprint, tmp_path / "table.rbl")

    preview_settings = {
        "TableColumn:name": [["Recording"]],
        "TableColumn:cell_kind": [[rrb.components.TableCellKind.Preview.value]],
        "TableColumnPreview:views": [[view.blueprint_path()]],
    }
    flag_settings = {
        "TableColumn:editable": [[True]],
        "TableColumn:visible": [[False]],
        "TableColumn:cell_kind": [[rrb.components.TableCellKind.Flag.value]],
    }
    assert set(data) == {
        f"/{view.blueprint_path()}",
        f"/{view.blueprint_path()}/ViewContents",
        "/table/layouts/table",
        "/table/layouts/table/columns/recording\\/link",
        "/table/layouts/table/columns/flag",
        "/table/layouts/cards",
        "/table/layouts/cards/fields/flag",
        "/table/layouts/cards/fields/recording\\/link",
    }
    assert data[f"/{view.blueprint_path()}"] == {
        "ViewBlueprint:class_identifier": [["2D"]],
        "ViewBlueprint:space_origin": [["/camera"]],
    }
    assert data[f"/{view.blueprint_path()}/ViewContents"] == {"ViewContents:query": [["$origin/**"]]}
    assert data["/table/layouts/table"] == {"TableLayout:column_order": [["recording/link", "flag"]]}
    assert data["/table/layouts/table/columns/recording\\/link"] == preview_settings
    assert data["/table/layouts/table/columns/flag"] == flag_settings
    assert data["/table/layouts/cards"] == {
        "CardLayout:field_order": [["flag", "recording/link"]],
        "CardLayout:title": [["recording/link"]],
        "CardLayout:link": [["recording/link"]],
    }
    assert data["/table/layouts/cards/fields/flag"] == flag_settings
    assert data["/table/layouts/cards/fields/recording\\/link"] == preview_settings


def test_table_blueprint_logs_shared_preview_once() -> None:
    view = rrb.Spatial2DView(origin="/camera")
    preview = rrb.table.Column("recording/link", cell=rrb.table.PreviewCell(view))
    blueprint = rrb.TableBlueprint(
        table_layout=rrb.table.TableLayout(columns=[preview]),
        card_layout=rrb.table.CardLayout(fields=[preview]),
    )
    stream = Mock(spec=rr.RecordingStream)

    with patch.object(view, "_log_to_stream") as log_view:
        blueprint._log_to_stream(stream)

    log_view.assert_called_once_with(stream)


@pytest.mark.parametrize("timeline", [None, "real_time"])
def test_table_blueprint_logs_generated_previews_config(timeline: str | None, tmp_path: Path) -> None:
    config = rrb.table.PreviewsConfig(timeline=timeline)
    blueprint = rrb.table.TableBlueprint(previews_config=config)
    data = _save_and_read(blueprint, tmp_path / "table.rbl")

    if timeline is None:
        assert "/table" not in data
    else:
        assert data["/table"] == {"PreviewsConfig:timeline": [[timeline]]}


@pytest.mark.parametrize(
    ("default_layout", "mode"),
    [(None, None), ("table", None), (None, "full"), ("cards", "compact")],
)
def test_table_blueprint_logs_root_settings(
    default_layout: Literal["table", "cards"] | None,
    mode: rrb.components.ColumnDisplayModeLike | None,
    tmp_path: Path,
) -> None:
    blueprint = rrb.table.TableBlueprint(
        card_layout=rrb.table.CardLayout(),
        default_layout=default_layout,
        column_display_mode=mode,
    )
    data = _save_and_read(blueprint, tmp_path / "table.rbl")

    expected: dict[str, list[object]] = {}
    if default_layout is not None:
        expected["TableBlueprint:layout"] = [[rrb.components.TableLayoutKind.auto(default_layout).value]]
    if mode is not None:
        expected["TableBlueprint:column_display_mode"] = [[rrb.components.ColumnDisplayMode.auto(mode).value]]
    if expected:
        assert data["/table"] == expected
    else:
        assert "/table" not in data


@pytest.mark.parametrize("table_layout", [None, rrb.table.TableLayout()])
def test_table_blueprint_logs_empty_table_layout(table_layout: rrb.table.TableLayout | None, tmp_path: Path) -> None:
    blueprint = rrb.table.TableBlueprint(table_layout=table_layout)

    data = _save_and_read(blueprint, tmp_path / "table.rbl")

    assert data == {"/table/layouts/table": {"TableLayout:column_order": [[]]}}


def test_column_skips_empty_settings() -> None:
    column = rrb.table.Column("source")
    stream = Mock(spec=rr.RecordingStream)

    column._log_to_stream(stream, "table/layouts/table/columns/source")

    stream.log.assert_not_called()


def test_table_blueprint_validates_inputs() -> None:
    view = rrb.Spatial2DView()

    with pytest.raises(ValueError, match="at least one view"):
        rrb.table.PreviewCell()
    with pytest.raises(TypeError, match="Rerun views"):
        rrb.table.PreviewCell("not a view")  # type: ignore[arg-type]
    with pytest.raises(ValueError, match="each source column at most once"):
        rrb.table.TableLayout(columns=[rrb.table.Column("a"), rrb.table.Column("a")])
    with pytest.raises(ValueError, match="at most one visible FlagCell"):
        rrb.table.CardLayout(
            fields=[
                rrb.table.Column("a", cell=rrb.table.FlagCell()),
                rrb.table.Column("b", cell=rrb.table.FlagCell()),
            ],
        )
    with pytest.raises(ValueError, match="requires card_layout"):
        rrb.table.TableBlueprint(default_layout="cards")
    with pytest.raises(ValueError, match="require FlagCell"):
        rrb.table.Column("a", editable=True)

    assert rrb.table.PreviewCell(view).views == (view,)


@pytest.mark.parametrize("visible", [None, True, False])
def test_card_layout_accepts_hidden_flags(visible: bool | None) -> None:
    fields = [
        rrb.table.Column("primary", cell=rrb.table.FlagCell(), visible=visible),
        rrb.table.Column("secondary", cell=rrb.table.FlagCell(), visible=False),
    ]

    layout = rrb.table.CardLayout(fields=fields)

    assert layout.fields == tuple(fields)
