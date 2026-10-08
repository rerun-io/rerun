"""
Configure catalog table layouts, columns, and segment previews.

⚠️ **The `rerun.blueprint.table` namespace is _unstable_ and may change significantly.**
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Literal

import rerun_bindings as bindings

from .._log import escape_entity_path_part
from ..recording_stream import RecordingStream
from ._table_cells import (
    EntryKindCell as EntryKindCell,
    FlagCell as FlagCell,
    LinkCell as LinkCell,
    PreviewCell as PreviewCell,
    ThumbnailCell as ThumbnailCell,
    _Cell,
)
from .archetypes import (
    CardLayout as CardLayoutArchetype,
    PreviewsConfig as PreviewsConfig,
    TableBlueprint as TableBlueprintArchetype,
    TableColumn as TableColumnArchetype,
    TableColumnPreview,
    TableLayout as TableLayoutArchetype,
)

if TYPE_CHECKING:
    from collections.abc import Sequence
    from pathlib import Path

    from .api import View
    from .components import ColumnDisplayModeLike

__all__ = [
    "CardLayout",
    "Column",
    "EntryKindCell",
    "FlagCell",
    "LinkCell",
    "PreviewCell",
    "PreviewsConfig",
    "TableBlueprint",
    "TableLayout",
    "ThumbnailCell",
]


def _validate_optional_string(value: object, name: str, *, allow_empty: bool = True) -> None:
    if value is not None and not isinstance(value, str):
        raise TypeError(f"{name} must be a string or None")
    if not allow_empty and value == "":
        raise ValueError(f"{name} must not be empty")


def _validate_optional_bool(value: object, name: str) -> None:
    if value is not None and not isinstance(value, bool):
        raise TypeError(f"{name} must be a bool or None")


def _validate_unique_sources(columns: Sequence[Column], context: str) -> tuple[Column, ...]:
    result = tuple(columns)
    if not all(isinstance(column, Column) for column in result):
        raise TypeError(f"{context} only accepts Column values")

    sources = [column.source for column in result]
    if len(sources) != len(set(sources)):
        raise ValueError(f"{context} accepts each source column at most once")
    return result


@dataclass(frozen=True)
class Column:
    """
    Configure a source column's appearance and behavior in one layout.

    Parameters
    ----------
    source:
        Physical Arrow column name, not a display label.
    name:
        Display label, overriding `column_display_mode`.
        If unset, the label is derived from `source`.
    editable:
        Allow editing; disabled by default.
        Requires write permission on a remote table with a column marked by `rerun:is_table_index` metadata.
        Only boolean values rendered with [`FlagCell`][rerun.blueprint.table.FlagCell] support editing.
    cell:
        Cell renderer.
        If unset, the Viewer infers it from the component or Arrow datatype.
    visible:
        Override visibility in this layout.
        Configured columns are visible by default.
        Set to `False` to hide a column while retaining its configuration and position.

    """

    source: str
    name: str | None = field(default=None, kw_only=True)
    editable: bool | None = field(default=None, kw_only=True)
    cell: LinkCell | ThumbnailCell | FlagCell | EntryKindCell | PreviewCell | None = field(default=None, kw_only=True)
    visible: bool | None = field(default=None, kw_only=True)

    def __post_init__(self) -> None:
        if not isinstance(self.source, str):
            raise TypeError("Column.source must be a string")
        if not self.source:
            raise ValueError("Column.source must not be empty")
        _validate_optional_string(self.name, "Column.name")
        _validate_optional_bool(self.editable, "Column.editable")
        _validate_optional_bool(self.visible, "Column.visible")
        if self.cell is not None and not isinstance(self.cell, _Cell):
            raise TypeError("Column.cell must be a concrete table cell renderer")
        if self.editable and not isinstance(self.cell, FlagCell):
            raise ValueError("Editable columns currently require FlagCell")

    def _log_to_stream(self, stream: RecordingStream, path: str) -> None:
        components = TableColumnArchetype(
            name=self.name,
            editable=self.editable,
            visible=self.visible,
            cell_kind=self.cell._kind if self.cell is not None else None,
        ).as_component_batches()
        if components:
            stream.log(path, components)

        if isinstance(self.cell, PreviewCell):
            stream.log(path, TableColumnPreview(views=[view.blueprint_path() for view in self.cell.views]))


@dataclass(frozen=True, kw_only=True)
class TableLayout:
    """
    Display table records as rows and columns.

    Parameters
    ----------
    columns:
        Columns to configure, in display order, with each source listed at most once.
        Listed columns appear first and are visible unless `visible=False`.
        Unlisted columns follow in default order and retain Viewer visibility defaults.

    """

    columns: Sequence[Column] = ()

    def __post_init__(self) -> None:
        object.__setattr__(self, "columns", _validate_unique_sources(self.columns, "TableLayout.columns"))

    # TODO(RR-4810): Add `auto_visible_columns` after the type definitions model it.


@dataclass(frozen=True, kw_only=True)
class CardLayout:
    """
    Display table records as cards.

    Parameters
    ----------
    title:
        Physical Arrow column name to use for card titles.
        If unset or not found in the table, uses the first visible string field.
    link:
        Physical Arrow column name containing the target to open when a card is activated.
        If unset or not found in the table, uses the first visible field with a
        [`PreviewCell`][rerun.blueprint.table.PreviewCell].
    fields:
        Fields to configure, in display order, with each source listed at most once.
        Listed fields are visible unless `visible=False`; unlisted fields are hidden.
        At most one visible [`FlagCell`][rerun.blueprint.table.FlagCell] is allowed;
        it appears in the card header rather than the labeled-field list.

    """

    title: str | None = None
    link: str | None = None
    fields: Sequence[Column] = ()

    def __post_init__(self) -> None:
        _validate_optional_string(self.title, "CardLayout.title", allow_empty=False)
        _validate_optional_string(self.link, "CardLayout.link", allow_empty=False)
        fields = _validate_unique_sources(self.fields, "CardLayout.fields")
        if sum(isinstance(column.cell, FlagCell) and column.visible is not False for column in fields) > 1:
            raise ValueError("CardLayout.fields accepts at most one visible FlagCell")
        object.__setattr__(self, "fields", fields)


@dataclass(frozen=True, kw_only=True)
class TableBlueprint:
    """
    Configure layouts and segment previews for a catalog table.

    This configures a table, not a Viewer viewport [`Blueprint`][rerun.blueprint.Blueprint].
    Save it as an `.rbl` file and register its server-accessible URI with
    `TableEntry.register_blueprint(uri)` or `DatasetEntry.register_blueprint(uri, segment_table=True)`.
    See [Configure table layouts and segment previews](https://rerun.io/docs/howto/visualization/configure-table-blueprints) for the full workflow.

    Parameters
    ----------
    table_layout:
        Row-and-column layout, available by default.
    card_layout:
        Card layout, disabled unless configured.
    default_layout:
        Initial layout: `"table"` or `"cards"`.
        If unset, uses cards when configured, otherwise the table layout.
        `"cards"` requires `card_layout`.
    column_display_mode:
        Column-name formatting in both layouts; defaults to compact formatting.
        Explicit `Column.name` labels take precedence.
    previews_config:
        Timeline selection shared by every [`PreviewCell`][rerun.blueprint.table.PreviewCell] in both layouts.
        If no timeline is specified, one is picked automatically, preferring custom over built-in timelines.

    """

    table_layout: TableLayout | None = None
    card_layout: CardLayout | None = None
    default_layout: Literal["table", "cards"] | None = None
    column_display_mode: ColumnDisplayModeLike | None = None
    previews_config: PreviewsConfig | None = None

    def __post_init__(self) -> None:
        if self.table_layout is not None and not isinstance(self.table_layout, TableLayout):
            raise TypeError("TableBlueprint.table_layout must be a TableLayout or None")
        if self.card_layout is not None and not isinstance(self.card_layout, CardLayout):
            raise TypeError("TableBlueprint.card_layout must be a CardLayout or None")
        _validate_optional_string(self.default_layout, "TableBlueprint.default_layout")
        if self.default_layout not in (None, "table", "cards"):
            raise ValueError("TableBlueprint.default_layout must be 'table', 'cards', or None")
        if self.default_layout == "cards" and self.card_layout is None:
            raise ValueError("default_layout='cards' requires card_layout to be configured")
        if self.previews_config is not None and not isinstance(self.previews_config, PreviewsConfig):
            raise TypeError("TableBlueprint.previews_config must be a PreviewsConfig or None")

    def save(self, application_id: str, path: str | Path | None = None) -> None:
        """
        Save this table blueprint to an `.rbl` file for catalog registration.

        Parameters
        ----------
        application_id:
            Application ID stored in the blueprint recording.
        path:
            Output file path; defaults to `<application_id>.rbl`.

        """
        if path is None:
            path = f"{application_id}.rbl"
        else:
            path = str(path)

        blueprint_stream = RecordingStream._from_native(
            bindings.new_blueprint(
                application_id=application_id,
                make_default=False,
                make_thread_default=False,
                default_enabled=True,
            ),
        )
        blueprint_stream.set_time("blueprint", sequence=0)
        self._log_to_stream(blueprint_stream)
        bindings.save_blueprint(path, blueprint_stream.to_native())

    def _log_to_stream(self, stream: RecordingStream) -> None:
        layouts = []
        if self.table_layout is not None:
            layouts.append(self.table_layout.columns)
        if self.card_layout is not None:
            layouts.append(self.card_layout.fields)
        views: dict[str, View] = {
            view.blueprint_path(): view
            for columns in layouts
            for column in columns
            if isinstance(column.cell, PreviewCell)
            for view in column.cell.views
        }

        for view in views.values():
            view._log_to_stream(stream)

        components = TableBlueprintArchetype(
            layout=self.default_layout, column_display_mode=self.column_display_mode
        ).as_component_batches()
        if components:
            stream.log("table", components)
        if self.previews_config is not None:
            components = self.previews_config.as_component_batches()
            if components:
                stream.log("table", components)

        table_layout = self.table_layout if self.table_layout is not None else TableLayout()
        stream.log(
            "table/layouts/table",
            TableLayoutArchetype(column_order=[column.source for column in table_layout.columns]),
        )
        for column in table_layout.columns:
            source = escape_entity_path_part(column.source)
            column._log_to_stream(stream, f"table/layouts/table/columns/{source}")

        if self.card_layout is not None:
            stream.log(
                "table/layouts/cards",
                CardLayoutArchetype(
                    field_order=[field.source for field in self.card_layout.fields],
                    title=self.card_layout.title,
                    link=self.card_layout.link,
                ),
            )
            for field in self.card_layout.fields:
                source = escape_entity_path_part(field.source)
                field._log_to_stream(stream, f"table/layouts/cards/fields/{source}")

    # TODO(RR-4810): Add sorting after the type definitions model it.
    # TODO(RR-4810): Add filters after the type definitions model them.
