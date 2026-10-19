from __future__ import annotations

from datetime import UTC, datetime, timedelta, timezone
from typing import TYPE_CHECKING, Literal
from unittest.mock import Mock, patch

import pytest
import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint.encodings import SortDirection
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


@pytest.mark.parametrize(
    ("sort_by", "direction"),
    [
        ("score", None),
        (rrb.table.SortBy("score"), None),
        (rrb.table.SortBy("score", direction="descending"), SortDirection.Descending),
    ],
)
def test_table_blueprint_logs_sort(
    sort_by: str | rrb.table.SortBy,
    direction: SortDirection | None,
    tmp_path: Path,
) -> None:
    blueprint = rrb.table.TableBlueprint(sort_by=sort_by)
    data = _save_and_read(blueprint, tmp_path / "table.rbl")

    expected_direction = None if direction is None else direction.value
    assert data["/table"] == {"TableBlueprint:sort_by": [[{"column": "score", "direction": expected_direction}]]}


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
    with pytest.raises(ValueError, match="sort_by must not be empty"):
        rrb.table.TableBlueprint(sort_by="")
    with pytest.raises(ValueError, match="require FlagCell"):
        rrb.table.Column("a", editable=True)

    assert rrb.table.PreviewCell(view).views == (view,)


def test_table_blueprint_logs_filters(tmp_path: Path) -> None:
    blueprint = rrb.table.TableBlueprint(filters=[rrb.table.col("score") >= 3, '"a" > "b"'])

    data = _save_and_read(blueprint, tmp_path / "table.rbl")

    assert data["/table"] == {"TableBlueprint:filters": [['"score" >= 3', '"a" > "b"']]}


# Each filter writes the SQL the Viewer parses back into an editable filter, as in the
# `column_filters_round_trip_through_sql` snapshot of `re_dataframe_ui`.
_COLUMN = 'Some "column"'
_C = '"Some ""column"""'
_TIMESTAMP = datetime(1973, 3, 3, 9, 46, 40, tzinfo=UTC)
_FILTER_SQL = [
    (rrb.table.col(_COLUMN).is_true(), f"{_C} = true"),
    (rrb.table.col(_COLUMN).is_false(), f"{_C} = false"),
    (rrb.table.col(_COLUMN).is_not_true(), f"({_C} = true) IS NOT TRUE"),
    (rrb.table.col(_COLUMN).is_not_false(), f"({_C} = false) IS NOT TRUE"),
    (rrb.table.col(_COLUMN).list.is_true(), f"any_match({_C}, x -> x = true)"),
    (rrb.table.col(_COLUMN).is_null(), f"{_C} IS NULL"),
    (
        rrb.table.col(_COLUMN).list.is_not_null(),
        f"({_C} IS NULL OR NOT any_match({_C}, x -> true) OR any_match({_C}, x -> x IS NULL)) IS NOT TRUE",
    ),
    (rrb.table.col(_COLUMN) == -42, f"{_C} = -42"),
    (rrb.table.col(_COLUMN).eq(-42), f"{_C} = -42"),
    (rrb.table.col(_COLUMN) != 7, f"({_C} = 7) IS NOT TRUE"),
    (rrb.table.col(_COLUMN).ne(7), f"({_C} = 7) IS NOT TRUE"),
    (rrb.table.col(_COLUMN).lt(7), f"{_C} < 7"),
    (rrb.table.col(_COLUMN).le(7), f"{_C} <= 7"),
    (rrb.table.col(_COLUMN).gt(7), f"{_C} > 7"),
    (rrb.table.col(_COLUMN).ge(7), f"{_C} >= 7"),
    (rrb.table.col(_COLUMN) < 7, f"{_C} < 7"),
    (rrb.table.col(_COLUMN) <= 7, f"{_C} <= 7"),
    (rrb.table.col(_COLUMN) > -10.5, f"{_C} > -10.5"),
    (rrb.table.col(_COLUMN).list >= 7, f"any_match({_C}, x -> x >= 7)"),
    (rrb.table.col(_COLUMN) == float("inf"), f"{_C} = CAST('inf' AS DOUBLE)"),
    (rrb.table.col(_COLUMN).str.contains("query"), f"{_C} ILIKE '%query%'"),
    (
        rrb.table.col(_COLUMN).str.does_not_contain(r"it's 100% a_b\c"),
        rf"({_C} ILIKE '%it''s 100\% a\_b\\c%' ESCAPE '\') IS NOT TRUE",
    ),
    (rrb.table.col(_COLUMN).str.starts_with("query"), f"{_C} ILIKE 'query%'"),
    (rrb.table.col(_COLUMN).list.str.ends_with("query"), f"any_match({_C}, x -> x ILIKE '%query')"),
    (
        rrb.table.col(_COLUMN).today(),
        f"{_C} >= date_trunc('day', now()) AND {_C} < date_trunc('day', now()) + INTERVAL '1 day'",
    ),
    (
        rrb.table.col(_COLUMN).yesterday(),
        f"{_C} >= date_trunc('day', now()) - INTERVAL '1 day' AND {_C} < date_trunc('day', now())",
    ),
    (rrb.table.col(_COLUMN).last_24_hours(), f"{_C} >= now() - INTERVAL '24 hours' AND {_C} < now()"),
    (
        rrb.table.col(_COLUMN).this_week(),
        f"{_C} >= date_trunc('week', now()) AND {_C} < date_trunc('week', now()) + INTERVAL '7 days'",
    ),
    (
        rrb.table.col(_COLUMN).list.not_last_week(),
        (
            f"(any_match({_C}, x -> x >= date_trunc('week', now()) - INTERVAL '7 days'"
            " AND x < date_trunc('week', now()))) IS NOT TRUE"
        ),
    ),
    (rrb.table.col(_COLUMN).before(_TIMESTAMP), f"{_C} < TIMESTAMPTZ '1973-03-03T09:46:40Z'"),
    (rrb.table.col(_COLUMN).after(_TIMESTAMP), f"{_C} >= TIMESTAMPTZ '1973-03-03T09:46:40Z'"),
    (
        rrb.table.col(_COLUMN).between(_TIMESTAMP, _TIMESTAMP + timedelta(hours=1)),
        f"{_C} >= TIMESTAMPTZ '1973-03-03T09:46:40Z' AND {_C} < TIMESTAMPTZ '1973-03-03T10:46:40Z'",
    ),
    (
        rrb.table.col(_COLUMN).not_between(_TIMESTAMP, _TIMESTAMP + timedelta(hours=1)),
        f"({_C} >= TIMESTAMPTZ '1973-03-03T09:46:40Z' AND {_C} < TIMESTAMPTZ '1973-03-03T10:46:40Z') IS NOT TRUE",
    ),
    (
        rrb.table.col(_COLUMN).not_today(),
        f"({_C} >= date_trunc('day', now()) AND {_C} < date_trunc('day', now()) + INTERVAL '1 day') IS NOT TRUE",
    ),
    (
        rrb.table.col(_COLUMN).not_yesterday(),
        f"({_C} >= date_trunc('day', now()) - INTERVAL '1 day' AND {_C} < date_trunc('day', now())) IS NOT TRUE",
    ),
    (
        rrb.table.col(_COLUMN).not_last_24_hours(),
        f"({_C} >= now() - INTERVAL '24 hours' AND {_C} < now()) IS NOT TRUE",
    ),
    (
        rrb.table.col(_COLUMN).not_this_week(),
        (f"({_C} >= date_trunc('week', now()) AND {_C} < date_trunc('week', now()) + INTERVAL '7 days') IS NOT TRUE"),
    ),
]


@pytest.mark.parametrize(("row_filter", "sql"), _FILTER_SQL)
def test_filters_write_viewer_sql(row_filter: rrb.table.Filter, sql: str) -> None:
    assert row_filter.sql == sql


def test_timestamp_filters_convert_to_utc() -> None:
    column = rrb.table.col("time")
    local = datetime(1973, 3, 3, 11, 46, 40, tzinfo=timezone(timedelta(hours=2)))

    assert column.before(local) == column.before(_TIMESTAMP)
    assert column.before(_TIMESTAMP.replace(tzinfo=None)) == column.before(_TIMESTAMP)


def test_filters_validate_inputs() -> None:
    with pytest.raises(ValueError, match="must not be empty"):
        rrb.table.col("")
    with pytest.raises(ValueError, match="must not be empty"):
        rrb.table.Filter(" ")
    with pytest.raises(ValueError, match="non-empty string"):
        rrb.table.col("task").str.contains("")
    with pytest.raises(TypeError, match="datetime"):
        rrb.table.col("time").before("2026-01-01")  # type: ignore[arg-type]
    with pytest.raises(TypeError, match="Can't compare"):
        _ = rrb.table.col("score") >= None  # type: ignore[operator]  # ty: ignore[unsupported-operator]
    with pytest.raises(TypeError, match="Can't compare"):
        _ = rrb.table.col("task") == "pick"
    with pytest.raises(TypeError, match="Can't compare"):
        rrb.table.col("success").eq(True)
    with pytest.raises(TypeError, match="Can't compare"):
        _ = rrb.table.col("time") > _TIMESTAMP  # type: ignore[operator]  # ty: ignore[unsupported-operator]
    with pytest.raises(AttributeError):
        _ = rrb.table.col("tags").list.list  # type: ignore[attr-defined]
    with pytest.raises(TypeError, match="sequence of filters"):
        rrb.table.TableBlueprint(filters='"a" > 1')
    with pytest.raises(TypeError, match="no truth value"):
        _ = not rrb.table.col("success").is_true()
    with pytest.raises(TypeError, match="no truth value"):
        _ = rrb.table.col("score") > 1 and rrb.table.col("score") < 3


@pytest.mark.parametrize("visible", [None, True, False])
def test_card_layout_accepts_hidden_flags(visible: bool | None) -> None:
    fields = [
        rrb.table.Column("primary", cell=rrb.table.FlagCell(), visible=visible),
        rrb.table.Column("secondary", cell=rrb.table.FlagCell(), visible=False),
    ]

    layout = rrb.table.CardLayout(fields=fields)

    assert layout.fields == tuple(fields)
